package dsp206.link

import dsp206.proto.Channel
import dsp206.proto.Decoded
import dsp206.proto.FrameDecoder
import dsp206.proto.Hid
import dsp206.proto.Names
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.factoryDefault
import dsp206.proto.isMuted
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** Link worker against the simulated device (never the real one). Port of crates/dsp206-link/tests/link.rs. */
class LinkTest {
    private val fast = LinkConfig(readTimeoutMs = 150, writeTimeoutMs = 150, meterIntervalMs = 20)
    private val events = LinkedBlockingQueue<Event>()
    private val links = mutableListOf<Link>()

    private fun open(mock: MockDevice, cfg: LinkConfig = fast): Link =
        Link.start(MockTransport(mock), cfg) { events.add(it) }.also { links += it }

    @AfterTest
    fun closeLinks() {
        links.forEach {
            it.close()
            it.join(1000)
        }
    }

    /** Collect events until [stop] returns true or the timeout expires. */
    private fun collect(timeoutMs: Long, stop: (Event) -> Boolean): List<Event> {
        val end = System.currentTimeMillis() + timeoutMs
        val out = mutableListOf<Event>()
        while (System.currentTimeMillis() < end) {
            val ev = events.poll(10, TimeUnit.MILLISECONDS) ?: continue
            out += ev
            if (stop(ev)) break
        }
        return out
    }

    private fun waitIdle(link: Link) {
        val end = System.currentTimeMillis() + 5000
        while (link.pending > 0 && System.currentTimeMillis() < end) Thread.sleep(5)
        Thread.sleep(30)
    }

    @Test
    fun readsEverythingFromDevice() {
        val mock = MockDevice()
        mock.state { presets[3].name = Names.field("Main PA", Program.PROGRAM_NAME_LEN) }
        val link = open(mock)
        link.readAll()
        val evs = collect(5000) { it is Event.PresetSlots }
        assertTrue(Event.Handshake(0x19) in evs)
        assertTrue(Event.Identity("DSP206 V0105P") in evs)
        assertTrue(Event.ProgramRead(factoryDefault()) in evs)
        assertTrue(Event.CurrentPreset(0) in evs)
        assertTrue(Event.PresetName(3, "Main PA") in evs)
        assertEquals(20, evs.count { it is Event.PresetName })
        assertEquals(Program.PAGE_COUNT, evs.count { it is Event.Page })
        assertFalse(evs.any { it is Event.Timeout || it is Event.ProgramIncomplete })
        // The program is read before the preset names (the device may go quiet).
        assertTrue(evs.indexOfFirst { it is Event.ProgramRead } < evs.indexOfFirst { it is Event.PresetName })
    }

    @Test
    fun settersReachTheDeviceAndAreAcknowledged() {
        val mock = MockDevice()
        val link = open(mock)
        link.send(Request.SetGain(Channel.OUT3, 200))
        link.send(Request.SetMute(Channel.OUT3, true))
        waitIdle(link)
        mock.state {
            assertEquals(200, program.gain(Channel.OUT3))
            assertTrue(program.isMuted(Channel.OUT3))
        }
        assertFalse(collect(100) { false }.any { it is Event.Unconfirmed })
    }

    @Test
    fun queuedSettersAreCoalesced() {
        val mock = MockDevice(MockOptions(latencyMs = 2))
        val link = open(mock)
        for (raw in 0..200) link.send(Request.SetGain(Channel.IN_A, raw))
        waitIdle(link)
        val gains = mock.state { received.filterIsInstance<Request.SetGain>().map { it.raw } }
        assertTrue(gains.size < 50, "${gains.size} setters reached the device")
        assertEquals(200, gains.last())
        assertEquals(200, mock.state { program.gain(Channel.IN_A) })
    }

    @Test
    fun silentDeviceIsDetectedAndMissingPagesRetried() {
        // Like the real unit over the network: answers a burst, then stops.
        val mock = MockDevice(MockOptions(replyLimit = 10))
        val link = open(mock)
        link.readProgram()
        val evs = collect(20_000) { it is Event.ProgramIncomplete }
        assertTrue(Event.HealthChanged(Health.SILENT) in evs)
        assertEquals((10 until Program.PAGE_COUNT).toList(), (evs.last() as Event.ProgramIncomplete).missing)
        // Device answers again: only the missing pages are requested.
        mock.state {
            repliesSent = 0
            received.clear()
        }
        link.retryMissingPages()
        val again = collect(5000) { it is Event.ProgramRead }
        assertTrue(Event.HealthChanged(Health.ANSWERING) in again)
        assertIs<Event.ProgramRead>(again.last())
        assertEquals(Program.PAGE_COUNT - 10, mock.state { received.count { it is Request.ReadProgramPage } })
    }

    @Test
    fun unacknowledgedSetterIsSentAgainThenReported() {
        val mock = MockDevice(MockOptions(silentWrites = true))
        val link = open(mock)
        val req = Request.SetPolarity(Channel.IN_B, true)
        link.send(req)
        val evs = collect(3000) { it is Event.Unconfirmed }
        assertEquals(Event.Unconfirmed(req), evs.last())
        // The setter still reached the device, 1 + retries times.
        assertEquals(1, mock.state { program.polarity(Channel.IN_B) })
        assertEquals(1 + fast.retries, mock.state { received.count { it == req } })
    }

    @Test
    fun meteringAndTrace() {
        val link = open(MockDevice())
        link.setMetering(true)
        val evs = collect(2000) { it is Event.MeterLevels }
        val m = (evs.last() as Event.MeterLevels).meters
        assertTrue(m.inputs.all { it > -40.0 && it < 10.0 })
        assertTrue(evs.any { it is Event.Trace && it.outgoing && it.bytes[5] == 0x40 })
        link.setMetering(false)
    }

    @Test
    fun recallThenFreshRead() {
        val mock = MockDevice()
        mock.state { presets[2].setGain(Channel.IN_A, 123) }
        val link = open(mock)
        link.send(Request.RecallPreset(2))
        link.readProgram()
        val p = (collect(5000) { it is Event.ProgramRead }.last() as Event.ProgramRead).program
        assertEquals(123, p.gain(Channel.IN_A))
        assertEquals(2, mock.state { currentPreset })
    }

    @Test
    fun storePresetKeepsTheCurrentProgramAndNamesIt() {
        val mock = MockDevice()
        mock.state { program.setGain(Channel.IN_A, 77) }
        val link = open(mock)
        storePresetSequence(1, "Club").forEach { link.send(it) }
        link.send(Request.GetPresetName(1))
        val evs = collect(5000) { it is Event.PresetName }
        assertTrue(Event.PresetName(1, "Club") in evs)
        mock.state {
            assertEquals(
                listOf(
                    Request.SetPresetName(Names.presetField("Club")),
                    Request.StorePreset(1),
                    Request.EndOfTransfer,
                    Request.GetPresetName(1),
                ),
                received,
            )
            // The current program stays: storing never loads a preset.
            assertEquals(77, program.gain(Channel.IN_A))
            assertEquals(77, presets[1].gain(Channel.IN_A))
            assertEquals(0, currentPreset)
        }
    }

    @Test
    fun slowStoreIsAwaitedAndSentOnce() {
        val mock = MockDevice(MockOptions(presetDelayMs = 600))
        // A setter would be given up after 150 ms; a store waits much longer.
        val link = open(mock, fast.copy(storeTimeoutMs = 3000))
        storePresetSequence(2, "Slow").forEach { link.send(it) }
        val evs = collect(3000) { it is Event.Trace && it.outgoing && it.bytes[5] == 0x12 }
        assertFalse(evs.any { it is Event.Unconfirmed })
        mock.state {
            assertEquals(1, received.count { it is Request.StorePreset })
            assertEquals("Slow", presets[2].nameText)
        }
    }

    @Test
    fun unansweredStoreIsReportedWithoutResending() {
        val mock = MockDevice(MockOptions(silentWrites = true))
        val link = open(mock, fast.copy(storeTimeoutMs = 200))
        link.send(Request.StorePreset(4))
        val evs = collect(3000) { it is Event.Unconfirmed }
        assertEquals(Event.Unconfirmed(Request.StorePreset(4)), evs.last())
        assertEquals(listOf<Request>(Request.StorePreset(4)), mock.state { received.toList() })
    }

    @Test
    fun closeSendsDisconnect() {
        val mock = MockDevice()
        val link = open(mock)
        link.send(Request.Identify)
        waitIdle(link)
        link.close()
        link.join(1000)
        assertFalse(link.isRunning)
        assertEquals(Request.Disconnect, mock.state { received.last() })
        assertTrue(Event.Closed(CloseReason.CLOSED) in collect(200) { it is Event.Closed })
    }

    @Test
    fun settersGoBeforeQueuedReads() {
        val mock = MockDevice()
        val link = open(mock)
        link.readAll()
        link.send(Request.SetMute(Channel.OUT6, true))
        collect(5000) { it is Event.PresetSlots }
        mock.state {
            // The handshake first, then the tap, then the long read.
            assertEquals(Request.Connect, received[0])
            assertEquals(Request.SetMute(Channel.OUT6, true), received[1])
            assertTrue(program.isMuted(Channel.OUT6))
        }
    }

    @Test
    fun nakMakesTheLinkSendAgain() {
        val mock = MockDevice(MockOptions(nakFirst = 1))
        val link = open(mock)
        val req = Request.SetMute(Channel.IN_A, true)
        link.send(req)
        waitIdle(link)
        mock.state {
            assertEquals(listOf<Request>(req, req), received)
            assertTrue(program.isMuted(Channel.IN_A))
        }
        assertFalse(collect(100) { false }.any { it is Event.Unconfirmed })
    }

    @Test
    fun gapSpacesOutFrames() {
        val link = open(MockDevice(), fast.copy(gapMs = 100))
        link.send(Request.Identify)
        link.send(Request.GetCurrentPreset)
        link.send(Request.GetLockState)
        link.send(Request.GetPresetSlots)
        val sent = mutableListOf<Long>()
        collect(3000) { ev ->
            if (ev is Event.Trace && ev.outgoing) sent += System.nanoTime()
            ev is Event.PresetSlots
        }
        assertEquals(4, sent.size)
        sent.zipWithNext().forEach { (a, b) -> assertTrue((b - a) / 1_000_000 >= 95, "frames only ${(b - a) / 1_000_000} ms apart") }
    }

    @Test
    fun worksOverTcp() {
        // A localhost server that hands every frame to the simulated device.
        val mock = MockDevice()
        ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress()).use { server ->
            val serving = thread(isDaemon = true) {
                server.accept().use { s ->
                    val dec = FrameDecoder()
                    val buf = ByteArray(512)
                    while (true) {
                        val n = s.getInputStream().read(buf)
                        if (n < 0) break
                        dec.push(buf, 0, n)
                        dec.drainAll().filterIsInstance<Decoded.Ok>().forEach { d ->
                            mock.handle(d.frame)?.let { s.getOutputStream().write(it.encode()) }
                        }
                    }
                }
            }
            val transport = TcpTransport.connect(InetSocketAddress(server.inetAddress, server.localPort), 1000)
            val link = Link.start(transport, fast) { events.add(it) }.also { links += it }
            link.readAll()
            val evs = collect(5000) { it is Event.PresetSlots }
            assertTrue(Event.ProgramRead(factoryDefault()) in evs)
            link.close()
            link.join(1000)
            serving.join(1000)
            assertEquals(Request.Disconnect, mock.state { received.last() })
        }
    }

    @Test
    fun worksOverHidReports() {
        // 64-byte reports, input padding full of stale bytes, as on the real unit (protocol.md §1.2).
        val mock = MockDevice()
        val inbox = LinkedBlockingQueue<ByteArray>()
        val written = mutableListOf<ByteArray>()
        val port = object : HidPort {
            override fun write(report: ByteArray) {
                assertEquals(Hid.REPORT_LEN, report.size)
                synchronized(written) { written += report }
                val dec = FrameDecoder()
                dec.push(report, 0, Hid.inputFrameLength(report))
                val frame = (dec.next() as Decoded.Ok).frame
                mock.handle(frame)?.let {
                    val r = ByteArray(Hid.REPORT_LEN) { 0x10 } // looks like frame starts
                    it.encode().copyInto(r)
                    inbox.add(r)
                }
            }

            override fun read(report: ByteArray, timeoutMs: Int): Int {
                val r = inbox.poll(timeoutMs.toLong(), TimeUnit.MILLISECONDS) ?: return 0
                r.copyInto(report)
                return r.size
            }

            override fun close() {}
        }
        val link = Link.start(HidTransport(port), fast) { events.add(it) }.also { links += it }
        link.readAll()
        val evs = collect(5000) { it is Event.PresetSlots }
        assertTrue(Event.ProgramRead(factoryDefault()) in evs)
        assertFalse(evs.any { it is Event.Timeout })
        // Output reports carry the frame, then zeros.
        val first = synchronized(written) { written.first() }
        assertEquals(dsp206.proto.Bytes.hex("10 02 00 01 01 10 10 03 11 00"), dsp206.proto.Bytes.copyOf(first, 0, 10))
    }

    @Test
    fun sequences() {
        val all = readAllSequence()
        assertEquals(listOf(Request.Connect, Request.Identify, Request.ReadProgramPage(0)), all.take(3))
        assertEquals(Request.GetPresetSlots, all.last())
        assertEquals(2 + 19 + 1 + 1 + 20 + 2, all.size)
        assertEquals(Request.EndOfTransfer, programReadSequence().last())
        assertEquals(
            listOf(Request.SetPresetName(Names.presetField("Club")), Request.StorePreset(3), Request.EndOfTransfer),
            storePresetSequence(3, "Club"),
        )
    }
}

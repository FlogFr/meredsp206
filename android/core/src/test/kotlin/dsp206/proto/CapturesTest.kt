package dsp206.proto

import dsp206.capture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Replays frames captured on a real DSP 206 (the JSON files in tools/re/captures), as
 * crates/dsp206-proto/tests/captures.rs does. These are the reference
 * vectors: if a change breaks them, the change is wrong.
 */
class CapturesTest {
    private val networkCaptures = listOf("default-program-partial", "dump-run12")
    private val usbCaptures = listOf(
        "usb-01-full-read", "usb-02-meters-30s", "usb-03-mute-out6",
        "usb-04-after-store-attempt", "usb-05-stored-presets", "usb-06-current-name",
    )

    private fun replies(name: String): List<Reply> =
        capture(name).filter { it.dir == "rx" }.map { Reply.parse(Frame.decode(it.bytes)) }

    private fun assembled(name: String): ProgramAssembler {
        val asm = ProgramAssembler()
        replies(name).filterIsInstance<Reply.ProgramPage>().forEach { asm.insert(it.page, it.data) }
        return asm
    }

    @Test
    fun everyCapturedFrameDecodes() {
        for (name in networkCaptures + usbCaptures) {
            val all = capture(name)
            assertTrue(all.size > 10, "$name: too few frames parsed")
            for (c in all) {
                val f = Frame.decode(c.bytes)
                assertTrue(f.encode().contentEquals(c.bytes), "$name: re-encoding differs")
                val expected = if (c.dir == "tx") 0 to 1 else 1 to 0
                assertEquals(expected, f.src to f.dst, name)
                Hid.outputReport(c.bytes) // every frame fits in one USB report
            }
        }
    }

    @Test
    fun networkCapturesOnlyContainReadOnlyRequests() {
        for (name in networkCaptures) {
            for (c in capture(name).filter { it.dir == "tx" }) {
                assertTrue(c.bytes[5].toInt() and 0xff in Codes.READ_ONLY, "$name: write command")
            }
        }
    }

    @Test
    fun streamDecoderReadsConcatenatedReplies() {
        val rx = capture("default-program-partial").filter { it.dir == "rx" }
        val stream = rx.fold(ByteArray(0)) { acc, c -> acc + c.bytes }
        val dec = FrameDecoder()
        var n = 0
        for (start in stream.indices step 7) {
            dec.push(stream, start, minOf(7, stream.size - start))
            dec.drainAll().forEach {
                assertTrue(it is Decoded.Ok)
                n++
            }
        }
        assertEquals(rx.size, n)
    }

    @Test
    fun identityAndPresetNames() {
        var names = 0
        for (r in replies("default-program-partial")) {
            when (r) {
                is Reply.Identity -> assertEquals("DSP206 V0105P", r.text)
                is Reply.Connected -> assertEquals(0x19, r.status)
                is Reply.PresetName -> {
                    assertEquals("Default Preset", r.name)
                    names++
                }
                else -> {}
            }
        }
        assertEquals(21, names)
    }

    @Test
    fun capturedPagesMatchFactoryDefaults() {
        val asm = assembled("default-program-partial")
        assertEquals(listOf(8, 9, 11, 12, 13, 14, 16, 17, 18), asm.missingPages())
        val p = Program.fromBytes(asm.raw())
        assertEquals(Bytes.of(0xff, 0x00), p.header)
        assertEquals("Default Preset", p.nameText)
        for ((k, ch) in Channel.INPUTS.withIndex()) {
            assertEquals(listOf("InA", "InB")[k], p.channelName(ch))
            assertEquals(0.0, Units.gainDb(p.gain(ch)))
            assertEquals(0 to 0, p.polarity(ch) to p.delay(ch))
            assertEquals(1 shl k, p.link(ch))
            assertTrue((0 until Program.GEQ_BANDS).all { Units.eqGainDb(p.geq(ch, it)) == 0.0 })
            assertEquals(listOf(41, 71, 101, 140, 170, 200, 240, 270), p.peqBands(ch).map { it.freq })
            assertTrue(p.peqBands(ch).all { it.gain == 120 && Units.qValue(it.q) == 3.0 && it.kind == 0 })
            assertEquals(0 to 300, p.highPass(ch).freq to p.lowPass(ch).freq)
            val g = p.gate(ch)
            assertEquals(Triple(0, 99, 99), Triple(g.attack, g.hold, g.release))
            assertEquals(-90.0, Units.thresholdDb(g.threshold))
        }
        val out1 = Channel.OUT1
        assertEquals("Out1", p.channelName(out1))
        assertEquals(listOf(1, 280, 280), listOf(p.routing(out1), p.mix(out1, 0), p.mix(out1, 1)))
        assertEquals(listOf(31, 63, 95, 127, 158, 190, 222, 253, 285), p.peqBands(out1).map { it.freq })
        val c = p.compressor(out1)
        assertEquals(listOf(0, 50, 500, 0), listOf(c.ratio, Units.timeMs(c.attack), Units.timeMs(c.release), c.knee))
        assertEquals(20.0, Units.thresholdDb(c.threshold))
        val l = p.limiter(out1)
        assertEquals(listOf(50, 500, 0), listOf(Units.timeMs(l.attack), Units.timeMs(l.release), l.knee))
        assertEquals(20.0, Units.thresholdDb(l.threshold))
        assertEquals(listOf(280, 0, 0, 1), listOf(p.gain(out1), p.polarity(out1), p.delay(out1), p.link(out1)))
        // Out2 name starts at the end of page 7.
        assertEquals(Bytes.ascii("Out2"), p.channelNameField(Channel.OUT2).slice(0, 4))
        // Out5 tail (page 15) and Out6 name.
        assertEquals(280 to (1 shl 4), p.gain(Channel.OUT5) to p.link(Channel.OUT5))
        assertEquals(Bytes.ascii("Out6"), p.channelNameField(Channel.OUT6).slice(0, 4))
    }

    @Test
    fun factoryDefaultMatchesEveryCapturedPage() {
        val factory = factoryDefault().toPages()
        var checked = 0
        for (r in replies("default-program-partial").filterIsInstance<Reply.ProgramPage>()) {
            assertEquals(factory[r.page].slice(0, r.data.size), r.data, "page ${r.page}")
            checked++
        }
        assertEquals(10, checked)
    }

    @Test
    fun usbSessionsAreFullyAnswered() {
        for (name in usbCaptures) {
            val all = capture(name)
            // One reply per request, except the unanswered 0x11 at the end.
            val tx = all.count { it.dir == "tx" }
            val rx = all.count { it.dir == "rx" }
            val silent = all.count { it.dir == "tx" && it.bytes[5].toInt() == 0x11 }
            assertEquals(tx, rx + silent, name)
        }
    }

    @Test
    fun usbFullReadIsTheFactoryDefaultProgram() {
        val asm = assembled("usb-01-full-read")
        assertTrue(asm.missingPages().isEmpty())
        assertEquals(factoryDefault(), asm.program())
    }

    @Test
    fun mistakenStoreLeftU01Loaded() {
        // After `0x26` + `0x20 01` (meant as a store) the unit runs U01, whose
        // name did not change: `0x20` loads (protocol.md §4.2).
        val after = replies("usb-04-after-store-attempt")
        assertTrue(Reply.CurrentPreset(1) in after)
        assertTrue(Reply.PresetName(1, "Default Preset") in after)
        assertTrue(Reply.CurrentPreset(0) in replies("usb-01-full-read"))
        for (c in capture("usb-04-after-store-attempt").filter { it.dir == "tx" }) {
            assertTrue(c.bytes[5].toInt() and 0xff in Codes.READ_ONLY)
        }
    }

    @Test
    fun presetNamesAndSlotsCountU01FromZero() {
        val stored = replies("usb-05-stored-presets")
        val slots = stored.filterIsInstance<Reply.PresetSlots>().first().slots
        assertEquals(20, slots.size)
        val names = stored.filterIsInstance<Reply.PresetName>().associate { it.index to it.name }
        assertEquals(21, names.size)
        for (i in 0 until slots.size) {
            val user = names.getValue(i + 1) != "Default Preset"
            assertEquals(user, slots[i] == 0xff, "U%02d".format(i + 1))
        }
        assertEquals("Full Stacks", names[1])
        assertEquals("Test Muted", names[2])
        // Index 21 (wire 20) is the current program's name, bytes 2..16 of page 0.
        val after = replies("usb-06-current-name")
        val page0 = after.filterIsInstance<Reply.ProgramPage>().first { it.page == 0 }
        val programName = Names.text(page0.data.slice(2, 16))
        assertEquals("Muted", programName)
        assertTrue(Reply.PresetName(21, programName) in after)
        assertEquals(programName, names[21])
    }

    @Test
    fun endOfTransferIsAcknowledged() {
        // 0x12 got `10 02 01 00 01 01 10 03 00` over USB (protocol.md §4.1).
        val all = capture("usb-01-full-read")
        val i = all.indexOfFirst { it.dir == "tx" && it.bytes[5].toInt() == 0x12 }
        assertEquals(Bytes.hex("10 02 01 00 01 01 10 03 00"), Bytes.copyOf(all[i + 1].bytes))
    }

    @Test
    fun usbReportsCarryOneFrameThenStalePadding() {
        // Raw 64-byte input reports from the unit (protocol.md §1.2).
        var stale = 0
        val reports = capture("usb-03-mute-out6").filter { it.report != null }
        assertTrue(reports.isNotEmpty())
        for (c in reports) {
            val report = c.report!!
            assertEquals(Hid.REPORT_LEN, report.size)
            assertEquals(c.bytes.size, Hid.inputFrameLength(report))
            val padding = report.copyOfRange(c.bytes.size, report.size)
            if ((0 until padding.size - 1).any { padding[it] == 0x10.toByte() && padding[it + 1] == 0x03.toByte() }) stale++
            // The last 4 bytes are past every frame the unit sends (60 bytes max).
            assertEquals(Bytes.hex("00 49 b4 53"), Bytes.copyOf(report, 60, 64))
        }
        assertEquals(3, stale, "the short replies (connect, two ACKs) and none else")
    }

    @Test
    fun usbMuteOut6ReadsBackInTheOutputMuteMask() {
        // memory-map.md §3: 0x382 = output mute mask, bit 5 = Out6.
        val masks = replies("usb-03-mute-out6").filterIsInstance<Reply.ProgramPage>().filter { it.page == 17 }
            .map { it.data.u16(0x382 - 850) }
        assertEquals(listOf(0x0000, 0x0020, 0x0000), masks)
    }
}

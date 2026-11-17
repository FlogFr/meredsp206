package dsp206.proto

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CommandTest {
    /** One example of every request with its expected payload (same table as the Rust tests). */
    private fun examples(): List<Pair<Request, String>> {
        val band = PeqBand(gain = 120, freq = 170, q = 35, kind = 0)
        return listOf(
            Request.Connect to "",
            Request.Disconnect to "",
            Request.EndOfTransfer to "",
            Request.Identify to "",
            Request.GetCurrentPreset to "",
            Request.GetLockState to "",
            Request.GetMeters to "",
            Request.GetPresetSlots to "",
            Request.GetPresetName(20) to "13",
            Request.ReadProgramPage(18) to "12",
            Request.SetPresetName(Bytes.ascii("Main PA 3-way") + Bytes.of(0)) to "4d 61 69 6e 20 50 41 20 33 2d 77 61 79 00",
            Request.StorePreset(3) to "03",
            Request.RecallPreset(3) to "03",
            Request.SetGain(Channel.OUT1, 280) to "02 18 01",
            Request.SetMute(Channel.OUT6, true) to "07 01",
            Request.SetPolarity(Channel.IN_A, true) to "00 01",
            Request.SetDelay(Channel.OUT2, 65280) to "03 00 ff",
            Request.SetDelayUnit(2) to "02",
            Request.SetPeqBand(Channel.IN_B, 7, band, true) to "01 07 78 00 aa 00 23 00 01",
            Request.SetPeqBypass(Channel.OUT3, false) to "04 00",
            Request.SetGeqBand(Channel.IN_A, 30, 240) to "00 1e f0 00",
            Request.SetGeqBypass(Channel.IN_B, true) to "01 01",
            Request.SetHighPass(Channel.OUT1, CutFilter(300, 9)) to "02 2c 01 09",
            Request.SetLowPass(Channel.OUT1, CutFilter(0, 19)) to "02 00 00 13",
            Request.SetGate(Channel.IN_A, Gate(0, 99, 99, 0)) to "00 00 00 63 00 63 00 00 00",
            Request.SetCompressor(Channel.OUT1, Compressor(0, 49, 499, 0, 220)) to "02 00 00 31 00 f3 01 00 00 dc 00",
            Request.SetLimiter(Channel.OUT1, Limiter(49, 499, 0, 220)) to "02 31 00 f3 01 00 00 dc 00",
            Request.SetRouting(Channel.OUT1, 3) to "02 03",
            Request.SetMixGain(Channel.OUT1, 1, 280) to "02 01 18 01",
            Request.SetLink(Channel.OUT1, 0x3f) to "02 3f",
            Request.CopyChannel(Channel.OUT1, Channel.OUT2) to "02 03",
            Request.SetChannelName(Channel.OUT1, Bytes.ascii("Sub L") + Bytes.zeros(3)) to "02 53 75 62 20 4c 00 00 00",
            Request.TestTone(ToneSource.SINE.raw, 17) to "03 11",
        )
    }

    @Test
    fun payloadsMatchProtocolDoc() {
        for ((req, expected) in examples()) assertEquals(expected, req.payload().hex(), "$req")
    }

    @Test
    fun parseInvertsPayload() {
        for ((req, _) in examples()) assertEquals(req, Request.parse(req.code, req.payload()), "$req")
        assertNull(Request.parse(0x34, Bytes.of(0x02, 0x18))) // truncated
        assertNull(Request.parse(0x34, Bytes.of(0x08, 0x18, 0x01))) // bad channel
        assertNull(Request.parse(0x10, Bytes.of(0x00))) // unexpected payload
        assertNull(Request.parse(0x77, Bytes.EMPTY))
    }

    @Test
    fun everyCodeIsUniqueAndHasAnExample() {
        val codes = examples().map { it.first.code }
        assertEquals(codes.size, codes.toSet().size, "two requests share a command code")
        assertEquals(Codes.ALL, codes.sorted(), "Codes.ALL is out of date, or a request lacks an example")
    }

    @Test
    fun everyRequestFitsInOneUsbReport() {
        // The editor sends one frame per 64-byte HID report (protocol.md §1.2).
        for ((req, _) in examples()) Hid.outputReport(req.frame(1).encode())
    }

    @Test
    fun framesAreWellFormed() {
        for ((req, _) in examples()) {
            val back = Frame.decode(req.frame(1).encode())
            assertEquals(Triple(0, 1, req.code), Triple(back.src, back.dst, back.cmd))
            assertEquals(req.payload(), back.data)
        }
        // A real capture: connect to device 1.
        assertEquals("10 02 00 01 01 10 10 03 11", Bytes.copyOf(Request.Connect.frame(1).encode()).hex())
    }

    @Test
    fun presetCodesFollowTheOriginalEditor() {
        // Load is 0x20 (0x426f8c), store is 0x21 sent after the 0x26 ACK
        // (0x422df1). They were swapped once, so a "store" loaded U01.
        assertEquals(0x20, Request.RecallPreset(0).code)
        assertEquals(0x21, Request.StorePreset(1).code)
        assertEquals(0x26, Request.SetPresetName(Names.presetField("")).code)
        assertEquals(Request.StorePreset(1), Request.parse(0x21, Bytes.of(1)))
        assertEquals(Request.RecallPreset(0), Request.parse(0x20, Bytes.of(0)))
        val storeU01 = Request.StorePreset(1).frame(1).encode()
        assertEquals("10 02 00 01 02 21 01", Bytes.copyOf(storeU01, 0, 7).hex())
    }

    @Test
    fun presetNamesCountFromZeroOnTheWire() {
        // U01 is `0x29 00`; its reply comes back as preset 1 again (protocol.md §4.2).
        assertEquals(Bytes.of(0), Request.GetPresetName(1).payload())
        assertEquals(Bytes.of(19), Request.GetPresetName(20).payload())
        assertEquals(Request.GetPresetName(1), Request.parse(0x29, Bytes.of(0)))
        fun reply(i: Int) = Reply.parse(Frame.request(0, 0x29, Bytes.of(i) + Bytes.ascii("Club          ")))
        assertEquals(Reply.PresetName(1, "Club"), reply(0))
        assertEquals(Reply.PresetName(20, "Club"), reply(19))
        assertEquals(Reply.PresetName(21, "Club"), reply(20))
    }

    @Test
    fun onlyDocumentedReadsAreReadOnly() {
        for ((req, _) in examples()) {
            assertEquals(req.code in Codes.READ_ONLY, req.isReadOnly, "$req")
            // A read-only request is never coalesced away.
            if (req.isReadOnly) assertNull(req.coalesceKey, "$req")
        }
        assertFalse(Request.EndOfTransfer.isReadOnly)
        assertFalse(Request.RecallPreset(0).isReadOnly)
        assertTrue(Request.ReadProgramPage(0).isReadOnly)
    }

    @Test
    fun coalescingKeys() {
        val a = Request.SetGain(Channel.OUT1, 1)
        val b = Request.SetGain(Channel.OUT1, 300)
        val c = Request.SetGain(Channel.OUT2, 1)
        assertEquals(a.coalesceKey, b.coalesceKey)
        assertNotEquals(a.coalesceKey, c.coalesceKey)
        fun band(n: Int) = Request.SetGeqBand(Channel.IN_A, n, 120)
        assertNotEquals(band(1).coalesceKey, band(2).coalesceKey)
        assertNull(Request.StorePreset(1).coalesceKey)
    }

    @Test
    fun replyCodes() {
        assertEquals(0x24, Request.ReadProgramPage(0).replyCode)
        assertEquals(0x13, Request.Identify.replyCode)
        assertEquals(Codes.ACK, Request.SetGain(Channel.IN_A, 0).replyCode)
        assertNull(Request.Disconnect.replyCode)
        assertNull(Request.EndOfTransfer.replyCode)
        // Every request that is not read-only, except the session frames, waits for an ACK.
        for ((req, _) in examples()) {
            val session = req == Request.Disconnect || req == Request.EndOfTransfer
            assertEquals(!req.isReadOnly && !session, req.isWrite, "$req")
        }
    }

    @Test
    fun channels() {
        assertNull(Channel.of(8))
        assertEquals(Channel.OUT1, Channel.output(1))
        assertNull(Channel.output(7))
        assertEquals(listOf("InA", "InB", "Out1", "Out2", "Out3", "Out4", "Out5", "Out6"), Channel.entries.map { it.label })
        assertTrue(Channel.IN_B.isInput && !Channel.OUT1.isInput)
        assertEquals(5, Channel.OUT6.domainBit)
        assertEquals(1, Channel.IN_B.domainBit)
    }

    @Test
    fun parsesCapturedReplies() {
        fun f(cmd: Int, data: Bytes) = Frame(1, 0, cmd, data)
        assertEquals(Reply.Connected(0x19), Reply.parse(f(0x10, Bytes.of(0x19))))
        assertEquals(Reply.Identity("DSP206 V0105P"), Reply.parse(f(0x13, Bytes.ascii("DSP206 V0105P"))))
        assertEquals(Reply.CurrentPreset(0), Reply.parse(f(0x14, Bytes.of(0))))
        assertEquals(Reply.PresetName(6, "Default Preset"), Reply.parse(f(0x29, Bytes.of(5) + Bytes.ascii("Default Preset"))))
        assertEquals(Reply.ProgramPage(3, Bytes.of(0xaa, 0xbb)), Reply.parse(f(0x24, Bytes.of(3, 0xaa, 0xbb))))
        assertEquals(Reply.Nak, Reply.parse(f(0x02, Bytes.EMPTY)))
        assertEquals(Reply.Ack, Reply.parse(f(0x01, Bytes.EMPTY)))
        assertIs<Reply.Other>(Reply.parse(f(0x77, Bytes.of(1))))
        assertIs<Reply.Other>(Reply.parse(f(0x10, Bytes.EMPTY)))
    }

    @Test
    fun parsesCapturedMeters() {
        // 0x40 reply payload captured on an idle unit (protocol.md §4.6).
        val data = Bytes.hex(
            "ad 36 14 ab 36 d7 00 00 00 00 00 00 ac 36 fa ab 36 d7 ad 36 14 aa 36 8e " +
                "ac 36 fa aa 36 8e 00 00 00 00 00 00 00 00",
        )
        val m = (Reply.parse(Frame(1, 0, 0x40, data)) as Reply.MeterLevels).meters
        assertTrue((m.inputs + m.outputs).all { it > -89.0 && it < -87.0 })
        assertEquals(Units.meterDbu(0xad, 0x36, 0x14), m.inputs[0])
        assertEquals(m.outputs[5], m.of(Channel.OUT6))
        assertEquals(0, m.status)
        assertNull(Meters.parse(data.slice(0, 31)))
    }
}

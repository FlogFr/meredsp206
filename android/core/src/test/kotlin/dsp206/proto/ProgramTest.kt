package dsp206.proto

import dsp206.noise
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ProgramTest {
    @Test
    fun layoutIsLossless() {
        for (seed in 1 until 20) {
            val bytes = noise(seed, Program.SIZE)
            assertTrue(Program.fromBytes(bytes).toBytes().contentEquals(bytes))
        }
        assertEquals(Program.SIZE, Program.blank().toBytes().size)
    }

    @Test
    fun fieldsSitAtDocumentedOffsets() {
        // memory-map.md §2: write a marker through each field, find it in the bytes.
        val p = Program.blank()
        p.name = Names.field("Default Preset", Program.PROGRAM_NAME_LEN)
        p.setGain(Channel.IN_B, 0xA1B2)
        p.setGeq(Channel.IN_A, 30, 0x0102)
        p.setPeq(Channel.IN_A, 7, PeqBand(0x1111, 0x2222, 0x33, 0x44))
        p.setHighPass(Channel.IN_A, CutFilter(0, 0x55))
        p.setRouting(Channel.OUT1, 0x0201)
        p.setCompressor(Channel.OUT1, Compressor(0, 0, 0, 0, 0xC3D4))
        p.setLink(Channel.OUT6, 0xBEEF)
        p.inputMute = 0x0003
        p.outputMute = 0x003F
        p.geqBypass = 0x7777
        val b = p.toBytes()
        fun w(o: Int) = (b[o].toInt() and 0xff) or ((b[o + 1].toInt() and 0xff) shl 8)
        assertEquals("Default Preset", String(b, 0x002, 14, Charsets.US_ASCII))
        assertEquals(0xA1B2, w(Program.INPUT_BASE + Program.INPUT_STRIDE + 0x84)) // InB gain at 0x120
        assertEquals(0x0102, w(0x020 + 30 * 2)) // InA GEQ band 31
        assertEquals(0x1111, w(0x05E + 7 * 6)) // InA PEQ 8 gain
        assertEquals(0x2222, w(0x05E + 7 * 6 + 2)) // InA PEQ 8 freq
        assertEquals(0x33, b[0x05E + 7 * 6 + 4].toInt())
        assertEquals(0x44, b[0x05E + 7 * 6 + 5].toInt())
        assertEquals(0x55, b[0x092].toInt()) // InA HPF slope
        assertEquals(0x0201, w(0x130)) // Out1 routing
        assertEquals(0xC3D4, w(0x17A)) // Out1 compressor threshold
        assertEquals(0xBEEF, w(Program.OUTPUT_BASE + 5 * Program.OUTPUT_STRIDE + 0x62)) // Out6 link at 0x37E
        assertEquals(0x0003, w(0x380))
        assertEquals(0x003F, w(0x382))
        assertEquals(0x7777, w(0x39E))
        assertEquals(0x37E, Program.OUTPUT_BASE + 5 * Program.OUTPUT_STRIDE + 0x62)
    }

    @Test
    fun everyAccessorRoundTrips() {
        val p = Program.fromBytes(noise(5, Program.SIZE))
        for (ch in Channel.entries) {
            val q = Program.blank()
            q.setChannelNameField(ch, p.channelNameField(ch))
            q.setGain(ch, p.gain(ch))
            q.setPolarity(ch, p.polarity(ch))
            q.setDelay(ch, p.delay(ch))
            q.setLink(ch, p.link(ch))
            q.setHighPass(ch, p.highPass(ch))
            q.setLowPass(ch, p.lowPass(ch))
            for (band in 0 until p.peqBandCount(ch)) q.setPeq(ch, band, p.peq(ch, band))
            if (ch.isInput) {
                q.setGate(ch, p.gate(ch))
                for (band in 0 until Program.GEQ_BANDS) q.setGeq(ch, band, p.geq(ch, band))
            } else {
                q.setRouting(ch, p.routing(ch))
                q.setMix(ch, 0, p.mix(ch, 0))
                q.setMix(ch, 1, p.mix(ch, 1))
                q.setCompressor(ch, p.compressor(ch))
                q.setLimiter(ch, p.limiter(ch))
            }
            // The accessors cover the whole channel block.
            val stride = if (ch.isInput) Program.INPUT_STRIDE else Program.OUTPUT_STRIDE
            val base = p.base(ch)
            assertTrue(
                p.toBytes().copyOfRange(base, base + stride).contentEquals(q.toBytes().copyOfRange(base, base + stride)),
                "$ch",
            )
        }
    }

    @Test
    fun rejectsShortInput() {
        assertEquals(ProgramError.TooShort(10), assertFailsWith<ProgramException> { Program.fromBytes(ByteArray(10)) }.error)
        // Page buffers are 950 bytes: the padding is ignored.
        Program.fromBytes(ByteArray(Program.PAGE_SIZE * Program.PAGE_COUNT))
    }

    @Test
    fun pagesRoundTripThroughAssembler() {
        val program = Program.fromBytes(noise(7, Program.SIZE))
        val pages = program.toPages()
        assertEquals(Program.PAGE_COUNT, pages.size)
        val asm = ProgramAssembler()
        for (i in pages.indices.reversed()) {
            assertNull(asm.program())
            asm.insert(i, pages[i])
        }
        assertTrue(asm.missingPages().isEmpty())
        assertEquals(program, asm.program())
        assertEquals(ProgramError.BadPage(19), assertFailsWith<ProgramException> { asm.insert(19, Bytes.EMPTY) }.error)
        assertEquals(ProgramError.PageTooLong(51), assertFailsWith<ProgramException> { asm.insert(0, Bytes.zeros(51)) }.error)
    }

    @Test
    fun names() {
        assertEquals("InA", Names.text(Bytes.ascii("InA") + Bytes.zeros(5)))
        assertEquals("Out6", Names.text(Bytes.ascii("Out6    ")))
        assertEquals(Bytes.ascii("Sub Left"), Names.field("Sub Left too long", 8))
        assertEquals(Bytes.of('?'.code, 0, 0, 0), Names.field("é", 4))
        assertEquals(Bytes.of('?'.code, 0), Names.field("🎵", 2))
        assertEquals(Bytes.ascii("Main PA       "), Names.presetField("Main PA "))
        assertEquals(Bytes.ascii(" ".repeat(14)), Names.presetField(""))
        assertEquals("Club", Names.text(Names.presetField("Club")))
    }

    @Test
    fun copiesAreIndependent() {
        val a = factoryDefault()
        val b = a.copy()
        b.setGain(Channel.IN_A, 1)
        assertFalse(a == b)
        assertEquals(280, a.gain(Channel.IN_A))
    }
}

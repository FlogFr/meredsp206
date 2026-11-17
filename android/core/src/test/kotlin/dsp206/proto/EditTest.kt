package dsp206.proto

import dsp206.noise
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class EditTest {
    /** A random program with valid (own-bit) link masks. */
    private fun randomProgram(seed: Int): Program {
        val p = Program.fromBytes(noise(seed, Program.SIZE))
        for (c in Channel.entries) {
            p.setLink(c, 1 shl c.domainBit)
            p.setPolarity(c, p.polarity(c) and 1)
            val name = p.channelNameField(c).toByteArray()
            for (i in name.indices) name[i] = ('A'.code + (name[i].toInt() and 0xff) % 26).toByte()
            p.setChannelNameField(c, Bytes.copyOf(name))
        }
        for (o in Channel.OUTPUTS) p.setRouting(o, p.routing(o) and 0xff)
        // Masks only hold bits that a setter can address.
        p.delayUnit = p.delayUnit and 0xff
        p.inputMute = p.inputMute and 0b11
        p.inputPeqBypass = p.inputPeqBypass and 0b11
        p.geqBypass = p.geqBypass and 0b11
        p.outputMute = p.outputMute and 0b11_1111
        p.outputPeqBypass = p.outputPeqBypass and 0b11_1111
        for (k in 0..1) p.setInputPeqBandBypass(k, p.inputPeqBandBypass(k) and 0xff)
        for (k in 0..5) p.setOutputPeqBandBypass(k, p.outputPeqBandBypass(k) and 0x1ff)
        return p
    }

    @Test
    fun factoryDefaultIsSane() {
        val p = factoryDefault()
        assertEquals("Default Preset", p.nameText)
        assertEquals("InA", p.channelName(Channel.IN_A))
        assertEquals("Out6", p.channelName(Channel.OUT6))
        for (c in Channel.entries) {
            assertEquals(0.0, Units.gainDb(p.gain(c)))
            assertFalse(p.isMuted(c))
            assertEquals(listOf(c), p.linkedChannels(c))
        }
    }

    @Test
    fun muteAndBypassBits() {
        val p = factoryDefault()
        assertTrue(p.apply(Request.SetMute(Channel.OUT6, true)))
        assertEquals(0b10_0000, p.outputMute)
        assertTrue(p.isMuted(Channel.OUT6) && !p.isMuted(Channel.OUT5))
        p.apply(Request.SetMute(Channel.IN_B, true))
        assertEquals(0b10, p.inputMute)
        p.apply(Request.SetPeqBypass(Channel.OUT1, true))
        assertEquals(1, p.outputPeqBypass)
        p.apply(Request.SetGeqBypass(Channel.IN_A, true))
        assertEquals(1, p.geqBypass)
        val band = p.peq(Channel.OUT2, 8)
        p.apply(Request.SetPeqBand(Channel.OUT2, 8, band, true))
        assertEquals(1 shl 8, p.outputPeqBandBypass(1))
        assertTrue(p.peqBandBypassed(Channel.OUT2, 8))
        // Setting the same value again changes nothing.
        assertFalse(p.apply(Request.SetMute(Channel.OUT6, true)))
    }

    @Test
    fun linkedChannelsFollowProcessingButNotNames() {
        val p = factoryDefault()
        // Link Out1 and Out2 (bits 0 and 1 on both).
        p.apply(Request.SetLink(Channel.OUT1, 0b11))
        p.apply(Request.SetLink(Channel.OUT2, 0b11))
        assertEquals(listOf(Channel.OUT1, Channel.OUT2), p.linkedChannels(Channel.OUT1))
        p.apply(Request.SetGain(Channel.OUT1, 200))
        assertEquals(listOf(200, 200, 280), listOf(Channel.OUT1, Channel.OUT2, Channel.OUT3).map { p.gain(it) })
        p.apply(Request.SetMute(Channel.OUT2, true))
        assertEquals(0b11, p.outputMute)
        p.apply(Request.SetChannelName(Channel.OUT1, Names.field("Sub", 8)))
        assertEquals("Sub" to "Out2", p.channelName(Channel.OUT1) to p.channelName(Channel.OUT2))
    }

    @Test
    fun invalidTargetsAreIgnored() {
        val p = factoryDefault()
        val gate = Gate(0, 0, 0, 0)
        assertFalse(p.apply(Request.SetGate(Channel.OUT1, gate)))
        assertFalse(p.apply(Request.SetGeqBand(Channel.OUT2, 0, 0)))
        assertFalse(p.apply(Request.SetGeqBand(Channel.IN_A, 31, 0)))
        assertFalse(p.apply(Request.SetPeqBand(Channel.IN_A, 8, PeqBand(0, 0, 0, 0), false)))
        assertFalse(p.apply(Request.SetRouting(Channel.IN_A, 3)))
        assertFalse(p.apply(Request.CopyChannel(Channel.IN_A, Channel.OUT1)))
        assertFalse(p.apply(Request.Identify))
        assertFailsWith<IllegalArgumentException> { p.gate(Channel.OUT1) }
        assertFailsWith<IllegalArgumentException> { p.routing(Channel.IN_A) }
    }

    @Test
    fun syncRequestsReproduceAnyProgram() {
        for (seed in 1 until 10) {
            val target = randomProgram(seed)
            val p = factoryDefault()
            for (r in target.syncRequests()) p.apply(r)
            // Fields without a setter: header, program name, unknown words.
            p.header = target.header
            p.name = target.name
            p.unknown394 = target.unknown394
            p.unknown396 = target.unknown396
            assertEquals(Bytes.copyOf(target.toBytes()), Bytes.copyOf(p.toBytes()), "seed $seed")
        }
    }

    @Test
    fun diffContainsOnlyChanges() {
        val a = factoryDefault()
        val b = a.copy()
        assertTrue(a.diffRequests(b).isEmpty())
        b.apply(Request.SetGain(Channel.OUT3, 300))
        b.apply(Request.SetMute(Channel.IN_A, true))
        assertEquals(listOf(Request.SetMute(Channel.IN_A, true), Request.SetGain(Channel.OUT3, 300)), a.diffRequests(b))
    }

    @Test
    fun copyChannelKeepsIdentityOfDestination() {
        val p = factoryDefault()
        p.apply(Request.SetGain(Channel.OUT1, 111))
        p.apply(Request.SetMute(Channel.OUT1, true))
        p.apply(Request.SetRouting(Channel.OUT1, 3))
        assertTrue(p.apply(Request.CopyChannel(Channel.OUT1, Channel.OUT4)))
        assertEquals(111, p.gain(Channel.OUT4))
        assertEquals(p.compressor(Channel.OUT1), p.compressor(Channel.OUT4))
        assertTrue(p.isMuted(Channel.OUT4))
        assertEquals("Out4", p.channelName(Channel.OUT4))
        assertEquals(2, p.routing(Channel.OUT4)) // routing not copied
        assertEquals(1 shl 3, p.link(Channel.OUT4))
    }

    @Test
    fun fileRoundTrip() {
        val p = randomProgram(3)
        val bytes = p.toFile()
        assertEquals(Bytes.ascii("DSP206P") + Bytes.of(1), Bytes.copyOf(bytes, 0, 8))
        assertEquals(p, programFromFile(bytes))
        assertEquals(ProgramError.BadFile, assertFailsWith<ProgramException> { programFromFile(bytes.copyOfRange(1, bytes.size)) }.error)
        assertEquals(ProgramError.BadFile, assertFailsWith<ProgramException> { programFromFile(bytes.copyOf(100)) }.error)
    }
}

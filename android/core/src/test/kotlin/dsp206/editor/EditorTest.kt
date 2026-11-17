package dsp206.editor

import dsp206.proto.Channel
import dsp206.proto.CutFilter
import dsp206.proto.Names
import dsp206.proto.PeqBand
import dsp206.proto.PeqType
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.factoryDefault
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotSame
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Port of the model tests of crates/dsp206-gtk/src/model.rs. */
class EditorModelTest {
    private fun gain(ch: Channel, raw: Int) = Request.SetGain(ch, raw)

    @Test
    fun faderDragIsOneUndoStep() {
        val m = EditorModel()
        for ((i, raw) in (200 until 260).withIndex()) assertEquals(gain(Channel.OUT1, raw), m.edit(gain(Channel.OUT1, raw), i * 20L))
        assertEquals(259, m.program.gain(Channel.OUT1))
        assertEquals(listOf<Request>(gain(Channel.OUT1, 280)), m.undo())
        assertEquals(factoryDefault(), m.program)
        assertFalse(m.canUndo)
        assertEquals(listOf<Request>(gain(Channel.OUT1, 259)), m.redo())
    }

    @Test
    fun separateParametersAreSeparateSteps() {
        val m = EditorModel()
        m.edit(gain(Channel.OUT1, 100), 0)
        m.edit(gain(Channel.OUT2, 100), 0)
        m.edit(gain(Channel.OUT1, 150), 5000)
        assertEquals(listOf<Request>(gain(Channel.OUT1, 100)), m.undo())
        assertEquals(listOf<Request>(gain(Channel.OUT2, 280)), m.undo())
        assertEquals(listOf<Request>(gain(Channel.OUT1, 280)), m.undo())
        assertTrue(m.undo().isEmpty())
    }

    @Test
    fun unchangedEditIsNotSent() {
        val m = EditorModel()
        assertNull(m.edit(gain(Channel.IN_A, 280), 0))
        assertFalse(m.canUndo)
    }

    @Test
    fun newEditClearsRedo() {
        val m = EditorModel()
        m.edit(gain(Channel.IN_A, 10), 0)
        m.undo()
        assertTrue(m.canRedo)
        m.edit(gain(Channel.IN_B, 10), 0)
        assertFalse(m.canRedo)
    }

    @Test
    fun everyEditPublishesANewProgram() {
        // Compose state compares by reference: the old snapshot must not change.
        val m = EditorModel()
        val before = m.program
        m.edit(gain(Channel.IN_A, 10), 0)
        assertNotSame(before, m.program)
        assertEquals(280, before.gain(Channel.IN_A))
    }

    @Test
    fun pagesOverlayAndLoad() {
        val m = EditorModel()
        val target = factoryDefault().also { it.setGain(Channel.IN_A, 333) }
        val page = 0x94 / Program.PAGE_SIZE // InA gain lives in page 2
        m.applyPage(page, target.toPages()[page])
        assertEquals(333, m.program.gain(Channel.IN_A))
        val other = factoryDefault().also { it.setGain(Channel.OUT6, 1) }
        val reqs = m.load(other)
        assertTrue(gain(Channel.OUT6, 1) in reqs && gain(Channel.IN_A, 280) in reqs)
        assertEquals(other, m.program)
        m.undo()
        assertEquals(333, m.program.gain(Channel.IN_A))
    }

    @Test
    fun storeNamesTheProgramAndChecksTheReadBack() {
        val m = EditorModel()
        assertEquals(
            listOf(
                Request.SetPresetName(Names.presetField("Club")),
                Request.StorePreset(1),
                Request.EndOfTransfer,
                Request.GetPresetName(1),
                Request.GetCurrentPreset,
            ),
            m.beginStore(1, "Club "),
        )
        assertEquals("Club", m.program.nameText)
        // Another slot's name does not end the store.
        assertNull(m.storeOutcome(2, "Club"))
        assertEquals(Notice.Stored(1, "Club"), m.storeOutcome(1, "Club"))
        assertNull(m.storeOutcome(1, "Club"))
        m.beginStore(3, "Live")
        assertEquals(Notice.NotStored(3, "Default Preset"), m.storeOutcome(3, "Default Preset"))
    }

    @Test
    fun unconfirmedPresetOperationsAreReported() {
        val m = EditorModel()
        m.beginStore(2, "A")
        assertEquals(Notice.StoreUnconfirmed(2), m.unconfirmed(Request.StorePreset(2)))
        assertNull(m.storeOutcome(2, "A"))
        assertNull(m.unconfirmed(Request.StorePreset(2)))
        assertEquals(Notice.RecallUnconfirmed(0), m.unconfirmed(Request.RecallPreset(0)))
        assertNull(m.unconfirmed(gain(Channel.IN_A, 1)))
    }

    @Test
    fun presetLabelsAndNames() {
        val m = EditorModel()
        assertEquals("Default Preset", m.presetNames[0])
        assertEquals("F00", EditorModel.presetLabel(0))
        assertEquals("U07", EditorModel.presetLabel(7))
        m.setPresetName(7, "Club")
        m.setPresetName(21, "Current") // the current program, not a slot
        assertEquals("Club", m.presetNames[7])
        assertEquals(21, m.presetNames.size)
    }
}

class FormatTest {
    @Test
    fun formats() {
        assertEquals("19.7 Hz", Format.freqIndex(0))
        assertEquals("1.00 kHz", Format.freqIndex(170))
        assertEquals("20.16 kHz", Format.freqIndex(300))
        assertEquals("203 Hz", Format.freqIndex(101))
        assertEquals("0.0 dB", Format.gain(280))
        assertEquals("+12.0 dB", Format.gain(400))
        assertEquals("-60.0 dB", Format.gain(0))
        assertEquals("-2.5 dB", Format.eqGain(95))
        assertEquals("+20.0 dB", Format.threshold(220))
        assertEquals("3.00", Format.q(35))
        assertEquals("128", Format.q(100))
        assertEquals("4.0:1", Format.ratio(9))
        assertEquals("∞:1", Format.ratio(15))
        assertEquals("1.00 ms", Format.delay(96, 0))
        assertEquals("3.435 m", Format.delay(960, 1))
        assertEquals("50 ms", Format.time(49))
        assertEquals("-∞", Format.dbu(-122.0))
        assertEquals("-12.3", Format.dbu(-12.3))
    }

    @Test
    fun commandNamesMatchTheProtocol() {
        assertEquals("store preset", Format.command(Request.StorePreset(1).code, true))
        assertEquals("recall preset", Format.command(Request.RecallPreset(1).code, true))
        assertEquals("ack", Format.command(0x01, false))
        assertEquals("", Format.command(0x01, true))
        assertEquals("program page", Format.command(0x24, false))
    }

    @Test
    fun parsesUserInput() {
        assertEquals(1000.0, Format.parseNumber("1k"))
        assertEquals(1500.0, Format.parseNumber("1.5 kHz"))
        assertEquals(-3.0, Format.parseNumber(" -3 dB"))
        assertEquals(2.5, Format.parseNumber("+2,5"))
        assertEquals(-6.0, Format.parseNumber("−6"))
        assertEquals(12.0, Format.parseNumber("12 ms"))
        assertNull(Format.parseNumber("abc"))
        assertNull(Format.parseNumber("1.5f"))
        assertNull(Format.parseNumber("NaN"))
    }
}

class ResponseTest {
    private fun close(a: Double, b: Double, tol: Double) = abs(a - b) <= tol

    @Test
    fun peakHasItsGainAtCentreAndIsFlatFarAway() {
        val b = Response.peqBiquad(PeqType.PEAK, 1000.0, 6.0, 1.4)
        assertTrue(close(b.magnitudeDb(1000.0), 6.0, 0.01))
        assertTrue(close(b.magnitudeDb(20.0), 0.0, 0.05))
        assertTrue(close(Response.peqBiquad(PeqType.PEAK, 1000.0, -6.0, 1.4).magnitudeDb(1000.0), -6.0, 0.01))
    }

    @Test
    fun shelvesAndPasses() {
        val ls = Response.peqBiquad(PeqType.LOW_SHELF, 200.0, 6.0, 0.71)
        assertTrue(close(ls.magnitudeDb(20.0), 6.0, 0.2))
        assertTrue(close(ls.magnitudeDb(10000.0), 0.0, 0.1))
        val hs = Response.peqBiquad(PeqType.HIGH_SHELF, 2000.0, -4.0, 0.71)
        assertTrue(close(hs.magnitudeDb(20000.0), -4.0, 0.3))
        val lp = Response.peqBiquad(PeqType.LOW_PASS_12, 1000.0, 0.0, 0.707)
        assertTrue(close(lp.magnitudeDb(1000.0), -3.0, 0.1))
        assertTrue(lp.magnitudeDb(10000.0) < -35.0)
        val hp = Response.peqBiquad(PeqType.HIGH_PASS_6, 1000.0, 0.0, 0.707)
        assertTrue(close(hp.magnitudeDb(1000.0), -3.0, 0.1))
        assertTrue(close(Response.peqBiquad(PeqType.ALLPASS_2, 1000.0, 0.0, 1.0).magnitudeDb(300.0), 0.0, 1e-9))
    }

    @Test
    fun crossoverSlopes() {
        // LK-24 is -6 dB at fc; BW-24 is -3 dB at fc.
        assertTrue(close(Response.cutDb(CutFilter(170, 9), true, 1000.0), -6.02, 0.01))
        val bw = CutFilter(170, 7)
        assertTrue(close(Response.cutDb(bw, false, 1000.0), -3.01, 0.01))
        // 24 dB/octave one octave away.
        assertTrue(close(Response.cutDb(bw, true, 500.0), -24.1, 0.3))
        // Defaults (19.7 Hz HP / 20.16 kHz LP, 6 dB/oct) barely touch the audio band.
        assertTrue(Response.cutDb(CutFilter(0, 0), true, 1000.0) > -0.01)
    }

    @Test
    fun factoryProgramIsNearlyFlat() {
        val band = PeqBand(120, 170, 35, 0)
        val b = Response.bandResponse(band)!!
        assertTrue(Response.freqAxis(50).all { abs(b.magnitudeDb(it)) < 1e-9 })
        assertNull(Response.bandResponse(band.copy(kind = 9)))
        // The default 6 dB/oct crossovers sit at 19.7 Hz and 20.16 kHz: about -3 dB at the edges.
        val total = Response.channelDb(factoryDefault(), Channel.OUT1, listOf(20.0, 100.0, 1000.0, 5000.0, 20000.0))
        assertTrue(close(total[0], -3.0, 0.1) && close(total[4], -3.0, 0.1), total.joinToString())
        assertTrue(total.slice(1..3).all { abs(it) < 0.5 }, total.joinToString())
    }

    @Test
    fun channelResponseFollowsBypass() {
        val p = factoryDefault()
        p.setPeq(Channel.IN_A, 4, PeqBand(180, 170, 16, 0)) // +6 dB at 1 kHz
        val axis = listOf(1000.0)
        assertTrue(close(Response.channelDb(p, Channel.IN_A, axis)[0], 6.0, 0.05))
        p.inputPeqBypass = 1
        assertTrue(abs(Response.channelDb(p, Channel.IN_A, axis)[0]) < 0.05)
    }

    @Test
    fun axisSpansAudioBand() {
        val a = Response.freqAxis(301)
        assertTrue(close(a[0], 20.0, 1e-9) && close(a[300], 20000.0, 1e-6))
    }
}

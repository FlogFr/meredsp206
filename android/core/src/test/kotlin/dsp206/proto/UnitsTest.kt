package dsp206.proto

import kotlin.math.abs
import kotlin.math.ln
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class UnitsTest {
    private fun close(a: Double, b: Double, tol: Double) = abs(a - b) <= tol

    @Test
    fun frequencyScaleMatchesOriginalApp() {
        // UI shows "19.7Hz" at index 0 and "20.16KHz" at index 300.
        assertTrue(close(Units.freqHz(0), 19.69, 0.01))
        assertEquals(1000.0, Units.freqHz(170))
        assertTrue(close(Units.freqHz(300), 20158.7, 0.1))
        // Factory input PEQ centres are within 2 % of 50, 100, 200, 500, 1k, 2k, 5k, 10k Hz.
        val idx = listOf(41, 71, 101, 140, 170, 200, 240, 270)
        val hz = listOf(50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0)
        idx.zip(hz).forEach { (i, f) -> assertTrue(close(Units.freqHz(i) / f, 1.0, 0.02), "index $i") }
    }

    @Test
    fun frequencyIndexRoundTrips() {
        for (i in 0..Units.FREQ_INDEX_MAX) assertEquals(i, Units.freqIndex(Units.freqHz(i)))
        assertEquals(0, Units.freqIndex(1.0))
        assertEquals(0, Units.freqIndex(0.0))
        assertEquals(Units.FREQ_INDEX_MAX, Units.freqIndex(1e6))
    }

    @Test
    fun gainScaleMatchesRs232Help() {
        assertEquals(-60.0, Units.gainDb(0))
        assertEquals(-20.0, Units.gainDb(80))
        assertEquals(0.0, Units.gainDb(280))
        assertEquals(12.0, Units.gainDb(400))
        assertEquals(-20.5, Units.gainDb(79))
        for (raw in 0..Units.GAIN_MAX) assertEquals(raw, Units.gainRaw(Units.gainDb(raw)))
        assertEquals(Units.GAIN_MAX, Units.gainRaw(99.0))
        assertEquals(0, Units.gainRaw(-99.0))
    }

    @Test
    fun eqGainAndThresholds() {
        assertEquals(0.0, Units.eqGainDb(120))
        assertEquals(-12.0, Units.eqGainDb(0))
        assertEquals(12.0, Units.eqGainDb(240))
        for (raw in 0..Units.EQ_GAIN_MAX) assertEquals(raw, Units.eqGainRaw(Units.eqGainDb(raw)))
        assertEquals(-90.0, Units.thresholdDb(0))
        assertEquals(0.0, Units.thresholdDb(Units.GATE_THRESHOLD_MAX))
        assertEquals(20.0, Units.thresholdDb(Units.DYN_THRESHOLD_MAX))
        assertEquals(Units.GATE_THRESHOLD_MAX, Units.thresholdRaw(20.0, Units.GATE_THRESHOLD_MAX))
        for (raw in 0..Units.DYN_THRESHOLD_MAX) {
            assertEquals(raw, Units.thresholdRaw(Units.thresholdDb(raw), Units.DYN_THRESHOLD_MAX))
        }
    }

    @Test
    fun timesAndDelay() {
        // Factory defaults: attack 49 → 50 ms, release 499 → 500 ms.
        assertEquals(50, Units.timeMs(49))
        assertEquals(500, Units.timeMs(499))
        assertEquals(499, Units.timeRaw(500, Units.RELEASE_MIN, Units.RELEASE_MAX))
        assertEquals(Units.RELEASE_MIN, Units.timeRaw(1, Units.RELEASE_MIN, Units.RELEASE_MAX))
        assertEquals(Units.ATTACK_MAX, Units.timeRaw(99_999, 0, Units.ATTACK_MAX))
        assertEquals(0, Units.timeRaw(0, 0, Units.ATTACK_MAX))
        assertEquals(1.0, Units.delayMs(96))
        assertEquals(680.0, Units.delayMs(Units.DELAY_MAX))
        assertEquals(Units.DELAY_MAX, Units.delayRaw(680.0))
        assertEquals(Units.DELAY_MAX, Units.delayRaw(1000.0))
        assertTrue(close(Units.delayMeters(96), 0.3435, 1e-9))
        assertTrue(close(Units.delayFeet(96), 0.3435 * 3.2808, 1e-9))
    }

    @Test
    fun lookupTables() {
        assertEquals(0.4, Units.qValue(0))
        assertEquals(1.0, Units.qValue(16))
        assertEquals(3.0, Units.qValue(35)) // factory PEQ Q index
        assertEquals(128.0, Units.qValue(100))
        assertNull(Units.qValue(101))
        assertTrue(Units.Q_TABLE.zipWithNext().all { (a, b) -> a < b })
        for (i in 0..100) assertEquals(i, Units.qIndex(Units.Q_TABLE[i]))
        assertTrue(ln(Units.Q_TABLE[0]) < 0)
        assertEquals(1.0, Units.ratio(0))
        assertEquals(4.0, Units.ratio(9))
        assertNull(Units.ratio(16))
        assertEquals(1000.0, Units.GEQ_BANDS_HZ[17])
        assertEquals(31, Units.GEQ_BANDS_HZ.size)
        assertEquals("BW -6", Units.slopeLabel(0))
        assertEquals("LK -24", Units.slopeLabel(9))
        assertEquals("LK -48", Units.slopeLabel(19))
        assertNull(Units.slopeLabel(20))
        assertEquals(PeqType.HIGH_SHELF, PeqType.fromRaw(2))
        assertNull(PeqType.fromRaw(9))
        assertEquals("Allpass2", PeqType.ALLPASS_2.label)
    }

    @Test
    fun meterDecodingMatchesCapture() {
        // Idle InA group from a captured 0x40 reply: about -87.75 dBu.
        assertTrue(close(Units.meterDbu(0xad, 0x36, 0x14), -87.75, 0.01))
        // Silence is floored, never -inf.
        assertTrue(close(Units.meterDbu(0, 0, 0), -122.0, 1e-9))
        // 1.0 linear (0x3f800000) is +18 dBu.
        assertTrue(close(Units.meterDbu(0x80, 0x3f, 0x00), 18.0, 1e-9))
    }
}

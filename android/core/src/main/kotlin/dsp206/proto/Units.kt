package dsp206.proto

import kotlin.math.abs
import kotlin.math.ln
import kotlin.math.log10
import kotlin.math.log2
import kotlin.math.pow

/** PEQ filter types (memory-map.md §4.5). */
enum class PeqType(val label: String) {
    PEAK("Peak"),
    LOW_SHELF("Low Shelf"),
    HIGH_SHELF("High Shelf"),
    LOW_PASS_6("LP -6dB"),
    LOW_PASS_12("LP -12dB"),
    HIGH_PASS_6("HP -6dB"),
    HIGH_PASS_12("HP -12dB"),
    ALLPASS_1("Allpass1"),
    ALLPASS_2("Allpass2"),
    ;

    /** True for the types whose gain value changes the response. */
    val hasGain: Boolean get() = this == PEAK || this == LOW_SHELF || this == HIGH_SHELF

    companion object {
        fun fromRaw(raw: Int): PeqType? = entries.getOrNull(raw)
    }
}

/** Crossover filter family. */
enum class FilterFamily(val tag: String) {
    BUTTERWORTH("BW"),
    BESSEL("BL"),
    LINKWITZ_RILEY("LK"),
}

data class Slope(val family: FilterFamily, val dbPerOctave: Int)

/** Raw device values ↔ engineering units (memory-map.md §4, protocol.md §4.6). */
object Units {
    /** Highest frequency index (memory-map.md §4.1). */
    const val FREQ_INDEX_MAX = 300

    /** Index of 1 kHz. */
    const val FREQ_INDEX_1K = 170

    /** Channel gain: raw 0..=400, 280 = 0 dB (memory-map.md §4.2). */
    const val GAIN_MAX = 400
    const val GAIN_UNITY = 280

    /** Mix gain is capped at 0 dB. */
    const val MIX_GAIN_MAX = GAIN_UNITY

    /** PEQ / GEQ gain: raw 0..=240, 120 = 0 dB. */
    const val EQ_GAIN_MAX = 240
    const val EQ_GAIN_FLAT = 120

    /** Thresholds: `(raw - 180) / 2` dB. Gate tops out at 0 dB, compressor/limiter at +20 dB. */
    const val GATE_THRESHOLD_MAX = 180
    const val DYN_THRESHOLD_MAX = 220

    /** Delay in samples at 96 kHz (memory-map.md §4.7). */
    const val DELAY_MAX = 65280
    const val SAMPLE_RATE_KHZ = 96.0

    /** Speed of sound and feet conversion used by the original app. */
    const val SPEED_OF_SOUND_M_PER_MS = 0.3435
    const val FEET_PER_METER = 3.2808
    const val KNEE_MAX = 12

    /** Attack 0..=998, hold 9..=998, release 9..=2999 (ms = raw + 1). */
    const val ATTACK_MAX = 998
    const val HOLD_MIN = 9
    const val HOLD_MAX = 998
    const val RELEASE_MIN = 9
    const val RELEASE_MAX = 2999

    /** Q values by index (memory-map.md §4.4). */
    val Q_TABLE: List<Double> = listOf(
        0.4, 0.42, 0.45, 0.47, 0.5, 0.53, 0.56, 0.59, 0.63, 0.67, 0.71, 0.75, 0.79, 0.84, 0.89, 0.94,
        1.0, 1.06, 1.12, 1.19, 1.26, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.4, 2.5, 2.7,
        2.8, 3.0, 3.2, 3.4, 3.6, 3.8, 4.0, 4.2, 4.5, 4.8, 5.0, 5.3, 5.7, 6.0, 6.3, 6.7, 7.1, 7.6, 8.0,
        8.5, 9.0, 9.5, 10.1, 10.7, 11.3, 12.0, 12.7, 13.5, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0,
        21.0, 23.0, 24.0, 25.0, 27.0, 29.0, 30.0, 32.0, 34.0, 36.0, 38.0, 40.0, 43.0, 45.0, 48.0, 51.0,
        54.0, 57.0, 60.0, 64.0, 68.0, 72.0, 76.0, 81.0, 85.0, 91.0, 96.0, 102.0, 108.0, 114.0, 121.0,
        128.0,
    )

    /** Compressor ratio by index (memory-map.md §4.8). The last entry means ∞. */
    val RATIO_TABLE: List<Double> = listOf(
        1.0, 1.1, 1.3, 1.5, 1.7, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0, 8.0, 10.0, 20.0, 20.1,
    )

    /** GEQ band centres in Hz (memory-map.md §4.3). */
    val GEQ_BANDS_HZ: List<Double> = listOf(
        20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
        500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
        8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
    )

    /** HPF/LPF slopes by raw index: (family, dB/octave) (memory-map.md §4.6). */
    val SLOPES: List<Slope> = run {
        val bw = FilterFamily.BUTTERWORTH
        val bl = FilterFamily.BESSEL
        val lk = FilterFamily.LINKWITZ_RILEY
        listOf(
            Slope(bw, 6), Slope(bl, 6), Slope(bw, 12), Slope(bl, 12), Slope(lk, 12),
            Slope(bw, 18), Slope(bl, 18), Slope(bw, 24), Slope(bl, 24), Slope(lk, 24),
            Slope(bw, 30), Slope(bl, 30), Slope(bw, 36), Slope(bl, 36), Slope(lk, 36),
            Slope(bw, 42), Slope(bl, 42), Slope(bw, 48), Slope(bl, 48), Slope(lk, 48),
        )
    }

    /** Label used by the original app, e.g. `"LK -24"`. */
    fun slopeLabel(raw: Int): String? = SLOPES.getOrNull(raw)?.let { "${it.family.tag} -${it.dbPerOctave}" }

    /** Rounds half away from zero like Rust's `f64::round`, then clamps to `0..max`. */
    private fun clampRound(x: Double, max: Int): Int {
        if (x.isNaN()) return 0
        val r = if (x >= 0) kotlin.math.floor(x + 0.5) else -kotlin.math.floor(-x + 0.5)
        return r.coerceIn(0.0, max.toDouble()).toInt()
    }

    /** Frequency index → Hz: `1000 · 2^((i − 170) / 30)`. */
    fun freqHz(index: Int): Double = 1000.0 * 2.0.pow((index - FREQ_INDEX_1K) / 30.0)

    /** Hz → nearest frequency index, clamped to 0..=300. */
    fun freqIndex(hz: Double): Int {
        if (hz <= 0.0) return 0
        return clampRound(30.0 * log2(hz / 1000.0) + FREQ_INDEX_1K, FREQ_INDEX_MAX)
    }

    /** Channel or mix gain raw → dB. */
    fun gainDb(raw: Int): Double = if (raw >= 80) (raw - GAIN_UNITY) / 10.0 else -60.0 + raw / 2.0

    /** dB → channel gain raw, clamped to 0..=400. */
    fun gainRaw(db: Double): Int =
        if (db >= -20.0) clampRound(db * 10.0 + GAIN_UNITY, GAIN_MAX) else clampRound((db + 60.0) * 2.0, GAIN_MAX)

    /** PEQ/GEQ gain raw → dB. */
    fun eqGainDb(raw: Int): Double = (raw - EQ_GAIN_FLAT) / 10.0

    fun eqGainRaw(db: Double): Int = clampRound(db * 10.0 + EQ_GAIN_FLAT, EQ_GAIN_MAX)

    /** Gate/compressor/limiter threshold raw → dB. */
    fun thresholdDb(raw: Int): Double = (raw - 180) / 2.0

    /** dB → threshold raw; pass [GATE_THRESHOLD_MAX] or [DYN_THRESHOLD_MAX]. */
    fun thresholdRaw(db: Double, max: Int): Int = clampRound(db * 2.0 + 180.0, max)

    /** Attack/hold/release raw → milliseconds. */
    fun timeMs(raw: Int): Int = raw + 1

    /** Milliseconds → attack/hold/release raw, clamped to `min..max` raw. */
    fun timeRaw(ms: Int, min: Int, max: Int): Int = (ms - 1).coerceIn(0, 0xffff).coerceIn(min, max)

    fun delayMs(raw: Int): Double = raw / SAMPLE_RATE_KHZ

    fun delayRaw(ms: Double): Int = clampRound(ms * SAMPLE_RATE_KHZ, DELAY_MAX)

    fun delayMeters(raw: Int): Double = delayMs(raw) * SPEED_OF_SOUND_M_PER_MS

    fun delayFeet(raw: Int): Double = delayMeters(raw) * FEET_PER_METER

    fun qValue(index: Int): Double? = Q_TABLE.getOrNull(index)

    /** Nearest Q index for a Q value (by log distance). */
    fun qIndex(q: Double): Int {
        val target = ln(maxOf(q, Double.MIN_VALUE))
        var best = 0
        for (i in Q_TABLE.indices) {
            if (abs(ln(Q_TABLE[i]) - target) < abs(ln(Q_TABLE[best]) - target)) best = i
        }
        return best
    }

    fun ratio(index: Int): Double? = RATIO_TABLE.getOrNull(index)

    /** One 3-byte meter group → linear amplitude (protocol.md §4.6). */
    fun meterAmplitude(g0: Int, g1: Int, g2: Int): Float {
        // Little-endian float32 assembled as [0x00, g2, g0, g1].
        val bits = ((g1 and 0xff) shl 24) or ((g0 and 0xff) shl 16) or ((g2 and 0xff) shl 8)
        return Float.fromBits(bits)
    }

    /** Floor used by the original app for silent channels. */
    const val METER_FLOOR = 1e-7

    /** One 3-byte meter group → dBu (`20·log10(x) + 18`). */
    fun meterDbu(g0: Int, g1: Int, g2: Int): Double {
        val x = meterAmplitude(g0, g1, g2).toDouble()
        val v = if (x.isNaN() || x < METER_FLOOR) METER_FLOOR else x
        return 20.0 * log10(v) + 18.0
    }
}

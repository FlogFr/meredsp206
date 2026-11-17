package dsp206.editor

import dsp206.proto.Channel
import dsp206.proto.CutFilter
import dsp206.proto.FilterFamily
import dsp206.proto.PeqBand
import dsp206.proto.PeqType
import dsp206.proto.Program
import dsp206.proto.Units
import dsp206.proto.geqBypassed
import dsp206.proto.peqBandBypassed
import dsp206.proto.peqBypassed
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.log10
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.math.tan

/**
 * Frequency response of a channel for the EQ graph (display only). Port of
 * crates/dsp206-gtk/src/response.rs.
 *
 * PEQ bands use RBJ biquads at 96 kHz. Crossover filters use analytic
 * Butterworth magnitudes; Linkwitz-Riley is a squared Butterworth and Bessel
 * is drawn with the Butterworth magnitude of the same order (approximation).
 */
object Response {
    private const val FS = 96_000.0

    /** Normalised biquad `b0 b1 b2 / a0 a1 a2`. */
    class Biquad(private val b: DoubleArray, private val a: DoubleArray) {
        fun magnitudeDb(f: Double): Double {
            val w = 2.0 * PI * f / FS
            fun eval(k: DoubleArray): Double {
                val re = k[0] + k[1] * cos(w) + k[2] * cos(2.0 * w)
                val im = -(k[1] * sin(w) + k[2] * sin(2.0 * w))
                return sqrt(re * re + im * im)
            }
            return 20.0 * log10(maxOf(eval(b) / eval(a), 1e-12))
        }
    }

    /** RBJ cookbook biquad for a PEQ band. */
    fun peqBiquad(kind: PeqType, f0: Double, gainDb: Double, q: Double): Biquad {
        val aLin = 10.0.pow(gainDb / 40.0)
        val w0 = 2.0 * PI * f0 / FS
        val c = cos(w0)
        val s = sin(w0)
        val alpha = s / (2.0 * q)
        return when (kind) {
            PeqType.PEAK -> Biquad(
                doubleArrayOf(1.0 + alpha * aLin, -2.0 * c, 1.0 - alpha * aLin),
                doubleArrayOf(1.0 + alpha / aLin, -2.0 * c, 1.0 - alpha / aLin),
            )
            PeqType.LOW_SHELF, PeqType.HIGH_SHELF -> {
                val sq = 2.0 * sqrt(aLin) * alpha
                val sign = if (kind == PeqType.LOW_SHELF) 1.0 else -1.0
                val ap = aLin + 1.0
                val am = aLin - 1.0
                Biquad(
                    doubleArrayOf(
                        aLin * (ap - sign * am * c + sq),
                        sign * 2.0 * aLin * (am - sign * ap * c),
                        aLin * (ap - sign * am * c - sq),
                    ),
                    doubleArrayOf(
                        ap + sign * am * c + sq,
                        -sign * 2.0 * (am + sign * ap * c),
                        ap + sign * am * c - sq,
                    ),
                )
            }
            PeqType.LOW_PASS_12 -> Biquad(
                doubleArrayOf((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
                doubleArrayOf(1.0 + alpha, -2.0 * c, 1.0 - alpha),
            )
            PeqType.HIGH_PASS_12 -> Biquad(
                doubleArrayOf((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
                doubleArrayOf(1.0 + alpha, -2.0 * c, 1.0 - alpha),
            )
            PeqType.LOW_PASS_6, PeqType.HIGH_PASS_6 -> {
                // Bilinear first-order sections.
                val k = tan(w0 / 2.0)
                if (kind == PeqType.LOW_PASS_6) {
                    Biquad(doubleArrayOf(k, k, 0.0), doubleArrayOf(k + 1.0, k - 1.0, 0.0))
                } else {
                    Biquad(doubleArrayOf(1.0, -1.0, 0.0), doubleArrayOf(k + 1.0, k - 1.0, 0.0))
                }
            }
            PeqType.ALLPASS_1, PeqType.ALLPASS_2 -> Biquad(doubleArrayOf(1.0, 0.0, 0.0), doubleArrayOf(1.0, 0.0, 0.0))
        }
    }

    /** Response of one stored PEQ band, or `null` when its raw values are invalid. */
    fun bandResponse(band: PeqBand): Biquad? {
        val kind = PeqType.fromRaw(band.kind) ?: return null
        val q = Units.qValue(band.q) ?: return null
        return peqBiquad(kind, Units.freqHz(band.freq), Units.eqGainDb(band.gain), q)
    }

    /** Crossover magnitude in dB ([highPass] selects the filter side). */
    fun cutDb(filter: CutFilter, highPass: Boolean, f: Double): Double {
        val slope = Units.SLOPES.getOrNull(filter.slope) ?: return 0.0
        val fc = Units.freqHz(filter.freq)
        val ratio = if (highPass) fc / f else f / fc
        val squared = slope.family == FilterFamily.LINKWITZ_RILEY
        val order = if (squared) slope.dbPerOctave / 12.0 else slope.dbPerOctave / 6.0
        val bw = -10.0 * log10(1.0 + ratio.pow(2.0 * order))
        return if (squared) 2.0 * bw else bw
    }

    /** Approximate response of a GEQ band (1/3-octave peaking filter). */
    fun geqBand(f0: Double, gainDb: Double): Biquad = peqBiquad(PeqType.PEAK, f0, gainDb, 4.32)

    /** [n] log-spaced frequencies from 20 Hz to 20 kHz. */
    fun freqAxis(n: Int): List<Double> = (0 until n).map { 20.0 * 1000.0.pow(it.toDouble() / maxOf(n - 1, 1)) }

    /** Total response of [ch] in dB at each frequency of [axis] (crossover, PEQ and GEQ). */
    fun channelDb(p: Program, ch: Channel, axis: List<Double>): DoubleArray {
        val hp = p.highPass(ch)
        val lp = p.lowPass(ch)
        val bands = if (p.peqBypassed(ch)) {
            emptyList()
        } else {
            (0 until p.peqBandCount(ch)).filter { !p.peqBandBypassed(ch, it) }.mapNotNull { bandResponse(p.peq(ch, it)) }
        }
        val geq = if (ch.isInput && !p.geqBypassed(ch)) {
            (0 until Program.GEQ_BANDS).mapNotNull { i ->
                val g = Units.eqGainDb(p.geq(ch, i))
                if (abs(g) > 0.01) geqBand(Units.GEQ_BANDS_HZ[i], g) else null
            }
        } else {
            emptyList()
        }
        return DoubleArray(axis.size) { i ->
            val f = axis[i]
            var db = cutDb(hp, true, f) + cutDb(lp, false, f)
            for (b in bands) db += b.magnitudeDb(f)
            for (b in geq) db += b.magnitudeDb(f)
            db
        }
    }
}

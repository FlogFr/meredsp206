package dsp206.editor

import dsp206.proto.Units
import java.util.Locale
import kotlin.math.abs

/** Display and parsing of raw device values (memory-map.md §4). Port of crates/dsp206-gtk/src/format.rs. */
object Format {
    private fun f(pattern: String, vararg args: Any): String = String.format(Locale.ROOT, pattern, *args)

    fun hz(v: Double): String = when {
        v >= 1000.0 -> f("%.2f kHz", v / 1000.0)
        v >= 100.0 -> f("%.0f Hz", v)
        else -> f("%.1f Hz", v)
    }

    fun freqIndex(i: Int): String = hz(Units.freqHz(i))

    fun db(v: Double): String = if (abs(v) < 0.05) "0.0 dB" else f("%+.1f dB", v)

    fun gain(raw: Int): String = db(Units.gainDb(raw))

    fun eqGain(raw: Int): String = db(Units.eqGainDb(raw))

    fun threshold(raw: Int): String = db(Units.thresholdDb(raw))

    fun q(index: Int): String {
        val v = Units.qValue(index) ?: return "?"
        return if (v >= 10.0) f("%.0f", v) else f("%.2f", v)
    }

    fun ratio(index: Int): String {
        val v = Units.ratio(index) ?: return "?"
        return if (index == 15) "∞:1" else f("%.1f:1", v)
    }

    fun ms(v: Int): String = "$v ms"

    fun time(raw: Int): String = ms(Units.timeMs(raw))

    /** Delay in the unit chosen on the device (0 = ms, 1 = m, 2 = ft). */
    fun delay(raw: Int, unit: Int): String = when (unit) {
        1 -> f("%.3f m", Units.delayMeters(raw))
        2 -> f("%.2f ft", Units.delayFeet(raw))
        else -> f("%.2f ms", Units.delayMs(raw))
    }

    /** Level meter text, e.g. `-12.3 dBu`, floored for silent channels. */
    fun dbu(v: Double): String = if (v < -99.0) "-∞" else f("%.1f", v)

    private val NUMBER = Regex("""[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?""")

    /** Parse a number with an optional unit suffix and `k` multiplier ("1k", "1.5 kHz", "-3 dB", "+2,5"). */
    fun parseNumber(text: String): Double? {
        var t = text.trim().lowercase(Locale.ROOT).replace(',', '.').replace('−', '-')
        for (suffix in listOf("hz", "db", "ms")) {
            while (t.endsWith(suffix)) t = t.removeSuffix(suffix)
        }
        t = t.trim()
        var mult = 1.0
        if (t.endsWith('k')) {
            t = t.removeSuffix("k").trim()
            mult = 1000.0
        }
        if (!NUMBER.matches(t)) return null
        return t.toDoubleOrNull()?.let { it * mult }
    }

    /** Short name of a command for the protocol log. */
    fun command(cmd: Int, outgoing: Boolean): String = when {
        cmd == 0x01 && !outgoing -> "ack"
        cmd == 0x02 && !outgoing -> "nak/busy"
        cmd == 0x24 && !outgoing -> "program page"
        else -> COMMANDS[cmd] ?: ""
    }

    private val COMMANDS = mapOf(
        0x10 to "connect", 0x11 to "disconnect", 0x12 to "end of transfer", 0x13 to "identify",
        0x14 to "current preset", 0x15 to "delay unit", 0x20 to "recall preset", 0x21 to "store preset",
        0x22 to "preset slots", 0x26 to "program name", 0x27 to "read page", 0x29 to "preset name",
        0x2A to "copy channel", 0x2C to "lock state", 0x30 to "compressor", 0x31 to "low-pass",
        0x32 to "high-pass", 0x33 to "peq band", 0x34 to "gain", 0x35 to "mute", 0x36 to "polarity",
        0x38 to "delay", 0x39 to "test tone", 0x3A to "routing", 0x3B to "link", 0x3C to "peq bypass",
        0x3D to "channel name", 0x3E to "gate", 0x3F to "limiter", 0x40 to "meters", 0x41 to "mix gain",
        0x48 to "geq band", 0x49 to "geq bypass",
    )
}

package dsp206.proto

/** A DSP 206 channel: 0 = InA, 1 = InB, 2..7 = Out1..Out6 (protocol.md §4). */
enum class Channel(val label: String) {
    IN_A("InA"),
    IN_B("InB"),
    OUT1("Out1"),
    OUT2("Out2"),
    OUT3("Out3"),
    OUT4("Out4"),
    OUT5("Out5"),
    OUT6("Out6"),
    ;

    val index: Int get() = ordinal

    val isInput: Boolean get() = ordinal < 2

    /** Bit index of this channel inside its domain mask (inputs 0–1, outputs 0–5). */
    val domainBit: Int get() = if (isInput) ordinal else ordinal - 2

    /** Channels of the same domain (inputs or outputs). */
    val domain: List<Channel> get() = if (isInput) INPUTS else OUTPUTS

    companion object {
        const val COUNT = 8
        val INPUTS: List<Channel> = listOf(IN_A, IN_B)
        val OUTPUTS: List<Channel> = listOf(OUT1, OUT2, OUT3, OUT4, OUT5, OUT6)

        fun of(index: Int): Channel? = entries.getOrNull(index)

        /** Output [n], 1-based (Out1..Out6). */
        fun output(n: Int): Channel? = if (n in 1..6) entries[n + 1] else null
    }
}

/** One parametric EQ band (memory-map.md §2.4). */
data class PeqBand(val gain: Int, val freq: Int, val q: Int, val kind: Int)

/** High-pass or low-pass filter settings. */
data class CutFilter(val freq: Int, val slope: Int)

data class Gate(val attack: Int, val hold: Int, val release: Int, val threshold: Int)

data class Compressor(val ratio: Int, val attack: Int, val release: Int, val knee: Int, val threshold: Int)

data class Limiter(val attack: Int, val release: Int, val knee: Int, val threshold: Int)

sealed class ProgramError {
    /** Fewer than [Program.SIZE] bytes were given. */
    data class TooShort(val size: Int) : ProgramError()

    /** Page number outside 0..19. */
    data class BadPage(val page: Int) : ProgramError()

    /** A page reply carried more than [Program.PAGE_SIZE] bytes. */
    data class PageTooLong(val size: Int) : ProgramError()

    /** Not a native program file (wrong magic or size). */
    data object BadFile : ProgramError()
}

class ProgramException(val error: ProgramError) : Exception(error.toString())

/** Fixed-size text fields (channel and program names). */
object Names {
    /** ASCII text of a NUL/space padded name field. */
    fun text(field: Bytes): String {
        val raw = field.toByteArray()
        val end = raw.indexOf(0).let { if (it < 0) raw.size else it }
        return String(raw, 0, end, Charsets.UTF_8).trimEnd()
    }

    /** Encode a name into an [n]-byte field, NUL padded, non-ASCII replaced by `?`. */
    fun field(text: String, n: Int): Bytes {
        val out = ByteArray(n)
        val points = text.codePoints().toArray()
        for (i in 0 until minOf(n, points.size)) {
            val c = points[i]
            out[i] = (if (c in 0x20..0x7e) c else '?'.code).toByte()
        }
        return Bytes.copyOf(out)
    }

    /**
     * Name sent with `0x26` before a store: space padded to 14 characters, as
     * the original editor does (`btnOKClick` `0x47bfe8`).
     */
    fun presetField(text: String): Bytes {
        val raw = field(text.trimEnd(), Program.PROGRAM_NAME_LEN).toByteArray()
        for (i in raw.indices) if (raw[i] == 0.toByte()) raw[i] = ' '.code.toByte()
        return Bytes.copyOf(raw)
    }
}

/**
 * The 928-byte program block (memory-map.md §2) and its paged transfer (§1).
 *
 * The block itself is the storage: every accessor reads or writes the
 * documented offset, so conversion to and from bytes is lossless by
 * construction. Instances are mutable; [copy] makes an independent snapshot.
 */
class Program private constructor(private val b: ByteArray) {
    fun toBytes(): ByteArray = b.copyOf()

    fun copy(): Program = Program(b.copyOf())

    override fun equals(other: Any?): Boolean = other is Program && b.contentEquals(other.b)

    override fun hashCode(): Int = b.contentHashCode()

    override fun toString(): String = "Program($nameText)"

    // ---- raw access ----------------------------------------------------

    fun u8(off: Int): Int = b[off].toInt() and 0xff

    fun setU8(off: Int, v: Int) {
        b[off] = v.toByte()
    }

    fun u16(off: Int): Int = u8(off) or (u8(off + 1) shl 8)

    fun setU16(off: Int, v: Int) {
        b[off] = v.toByte()
        b[off + 1] = (v shr 8).toByte()
    }

    private fun bytesAt(off: Int, n: Int): Bytes = Bytes.copyOf(b, off, off + n)

    private fun setBytesAt(off: Int, n: Int, v: Bytes) {
        require(v.size == n) { "field needs $n bytes, got ${v.size}" }
        v.copyInto(b, off)
    }

    // ---- header --------------------------------------------------------

    /** Bytes 0..2, meaning unknown (observed `ff 00`). */
    var header: Bytes
        get() = bytesAt(0, 2)
        set(v) = setBytesAt(0, 2, v)

    /** Program name field, 14 bytes at `0x002`. */
    var name: Bytes
        get() = bytesAt(NAME_OFFSET, PROGRAM_NAME_LEN)
        set(v) = setBytesAt(NAME_OFFSET, PROGRAM_NAME_LEN, v)

    val nameText: String get() = Names.text(name)

    // ---- channel blocks (memory-map.md §2.2, §2.3) -----------------------

    /** Absolute offset of a channel block. */
    fun base(ch: Channel): Int =
        if (ch.isInput) INPUT_BASE + ch.index * INPUT_STRIDE else OUTPUT_BASE + (ch.index - 2) * OUTPUT_STRIDE

    private fun off(ch: Channel, input: Int, output: Int) = base(ch) + if (ch.isInput) input else output

    fun peqBandCount(ch: Channel): Int = if (ch.isInput) INPUT_PEQ_BANDS else OUTPUT_PEQ_BANDS

    fun channelNameField(ch: Channel): Bytes = bytesAt(base(ch), NAME_LEN)

    fun setChannelNameField(ch: Channel, v: Bytes) = setBytesAt(base(ch), NAME_LEN, v)

    fun channelName(ch: Channel): String = Names.text(channelNameField(ch))

    fun gain(ch: Channel): Int = u16(off(ch, 0x84, 0x5C))

    fun setGain(ch: Channel, v: Int) = setU16(off(ch, 0x84, 0x5C), v)

    fun polarity(ch: Channel): Int = u16(off(ch, 0x86, 0x5E))

    fun setPolarity(ch: Channel, v: Int) = setU16(off(ch, 0x86, 0x5E), v)

    fun delay(ch: Channel): Int = u16(off(ch, 0x88, 0x60))

    fun setDelay(ch: Channel, v: Int) = setU16(off(ch, 0x88, 0x60), v)

    fun link(ch: Channel): Int = u16(off(ch, 0x8A, 0x62))

    fun setLink(ch: Channel, v: Int) = setU16(off(ch, 0x8A, 0x62), v)

    fun highPass(ch: Channel): CutFilter = CutFilter(u16(off(ch, 0x7E, 0x0E)), u8(off(ch, 0x82, 0x12)))

    fun setHighPass(ch: Channel, f: CutFilter) {
        setU16(off(ch, 0x7E, 0x0E), f.freq)
        setU8(off(ch, 0x82, 0x12), f.slope)
    }

    fun lowPass(ch: Channel): CutFilter = CutFilter(u16(off(ch, 0x80, 0x10)), u8(off(ch, 0x83, 0x13)))

    fun setLowPass(ch: Channel, f: CutFilter) {
        setU16(off(ch, 0x80, 0x10), f.freq)
        setU8(off(ch, 0x83, 0x13), f.slope)
    }

    private fun peqOff(ch: Channel, band: Int): Int {
        require(band in 0 until peqBandCount(ch)) { "no PEQ band $band on $ch" }
        return off(ch, 0x4E, 0x14) + band * 6
    }

    fun peq(ch: Channel, band: Int): PeqBand {
        val o = peqOff(ch, band)
        return PeqBand(u16(o), u16(o + 2), u8(o + 4), u8(o + 5))
    }

    fun setPeq(ch: Channel, band: Int, v: PeqBand) {
        val o = peqOff(ch, band)
        setU16(o, v.gain)
        setU16(o + 2, v.freq)
        setU8(o + 4, v.q)
        setU8(o + 5, v.kind)
    }

    fun peqBands(ch: Channel): List<PeqBand> = (0 until peqBandCount(ch)).map { peq(ch, it) }

    // Input-only fields.

    private fun inputOff(ch: Channel, rel: Int): Int {
        require(ch.isInput) { "$ch is not an input" }
        return base(ch) + rel
    }

    fun gate(ch: Channel): Gate {
        val o = inputOff(ch, 0x08)
        return Gate(u16(o), u16(o + 2), u16(o + 4), u16(o + 6))
    }

    fun setGate(ch: Channel, g: Gate) {
        val o = inputOff(ch, 0x08)
        setU16(o, g.attack)
        setU16(o + 2, g.hold)
        setU16(o + 4, g.release)
        setU16(o + 6, g.threshold)
    }

    fun geq(ch: Channel, band: Int): Int {
        require(band in 0 until GEQ_BANDS) { "no GEQ band $band" }
        return u16(inputOff(ch, 0x10) + band * 2)
    }

    fun setGeq(ch: Channel, band: Int, v: Int) {
        require(band in 0 until GEQ_BANDS) { "no GEQ band $band" }
        setU16(inputOff(ch, 0x10) + band * 2, v)
    }

    // Output-only fields.

    private fun outputOff(ch: Channel, rel: Int): Int {
        require(!ch.isInput) { "$ch is not an output" }
        return base(ch) + rel
    }

    /** Bit 0 = InA, bit 1 = InB. */
    fun routing(ch: Channel): Int = u16(outputOff(ch, 0x08))

    fun setRouting(ch: Channel, v: Int) = setU16(outputOff(ch, 0x08), v)

    /** Mix gain from InA (`input` 0) or InB (1). */
    fun mix(ch: Channel, input: Int): Int {
        require(input in 0..1)
        return u16(outputOff(ch, 0x0A) + input * 2)
    }

    fun setMix(ch: Channel, input: Int, v: Int) {
        require(input in 0..1)
        setU16(outputOff(ch, 0x0A) + input * 2, v)
    }

    fun compressor(ch: Channel): Compressor {
        val o = outputOff(ch, 0x4A)
        return Compressor(u16(o), u16(o + 2), u16(o + 4), u16(o + 6), u16(o + 8))
    }

    fun setCompressor(ch: Channel, c: Compressor) {
        val o = outputOff(ch, 0x4A)
        setU16(o, c.ratio)
        setU16(o + 2, c.attack)
        setU16(o + 4, c.release)
        setU16(o + 6, c.knee)
        setU16(o + 8, c.threshold)
    }

    fun limiter(ch: Channel): Limiter {
        val o = outputOff(ch, 0x54)
        return Limiter(u16(o), u16(o + 2), u16(o + 4), u16(o + 6))
    }

    fun setLimiter(ch: Channel, l: Limiter) {
        val o = outputOff(ch, 0x54)
        setU16(o, l.attack)
        setU16(o + 2, l.release)
        setU16(o + 4, l.knee)
        setU16(o + 6, l.threshold)
    }

    // ---- global masks (memory-map.md §2.5) -------------------------------

    var inputMute: Int
        get() = u16(0x380)
        set(v) = setU16(0x380, v)
    var outputMute: Int
        get() = u16(0x382)
        set(v) = setU16(0x382, v)

    /** PEQ band bypass mask of input `k` (0..1). */
    fun inputPeqBandBypass(k: Int): Int = u16(0x384 + 2 * k.also { require(it in 0..1) })

    fun setInputPeqBandBypass(k: Int, v: Int) = setU16(0x384 + 2 * k.also { require(it in 0..1) }, v)

    /** PEQ band bypass mask of output `k` (0..5). */
    fun outputPeqBandBypass(k: Int): Int = u16(0x388 + 2 * k.also { require(it in 0..5) })

    fun setOutputPeqBandBypass(k: Int, v: Int) = setU16(0x388 + 2 * k.also { require(it in 0..5) }, v)

    var unknown394: Int
        get() = u16(0x394)
        set(v) = setU16(0x394, v)
    var unknown396: Int
        get() = u16(0x396)
        set(v) = setU16(0x396, v)

    /** 0 = ms, 1 = m, 2 = ft. */
    var delayUnit: Int
        get() = u16(0x398)
        set(v) = setU16(0x398, v)
    var inputPeqBypass: Int
        get() = u16(0x39A)
        set(v) = setU16(0x39A, v)
    var outputPeqBypass: Int
        get() = u16(0x39C)
        set(v) = setU16(0x39C, v)
    var geqBypass: Int
        get() = u16(0x39E)
        set(v) = setU16(0x39E, v)

    /** Split the block into the 19 pages of 50 bytes sent by `0x24` replies. */
    fun toPages(): List<Bytes> {
        val padded = b.copyOf(PAGE_SIZE * PAGE_COUNT)
        return (0 until PAGE_COUNT).map { Bytes.copyOf(padded, it * PAGE_SIZE, (it + 1) * PAGE_SIZE) }
    }

    companion object {
        const val SIZE = 928
        const val PAGE_SIZE = 50
        const val PAGE_COUNT = 19
        const val INPUT_COUNT = 2
        const val OUTPUT_COUNT = 6
        const val INPUT_PEQ_BANDS = 8
        const val OUTPUT_PEQ_BANDS = 9
        const val GEQ_BANDS = 31
        const val NAME_LEN = 8
        const val PROGRAM_NAME_LEN = 14
        const val NAME_OFFSET = 0x002

        /** Absolute offsets of the channel blocks. */
        const val INPUT_BASE = 0x010
        const val INPUT_STRIDE = 0x8C
        const val OUTPUT_BASE = 0x128
        const val OUTPUT_STRIDE = 0x64
        const val GLOBAL_BASE = 0x380

        /** An all-zero block. */
        fun blank(): Program = Program(ByteArray(SIZE))

        /** Parse a program block. Extra trailing bytes (page padding) are ignored. */
        fun fromBytes(bytes: ByteArray): Program {
            if (bytes.size < SIZE) throw ProgramException(ProgramError.TooShort(bytes.size))
            return Program(bytes.copyOf(SIZE))
        }
    }
}

/** Collects `0x24` page replies until the program is complete. */
class ProgramAssembler {
    private val raw = ByteArray(Program.PAGE_SIZE * Program.PAGE_COUNT)
    private val received = BooleanArray(Program.PAGE_COUNT)

    /** Store one page ([data] excludes the leading page number). */
    fun insert(page: Int, data: Bytes) {
        if (page !in 0 until Program.PAGE_COUNT) throw ProgramException(ProgramError.BadPage(page))
        if (data.size > Program.PAGE_SIZE) throw ProgramException(ProgramError.PageTooLong(data.size))
        data.copyInto(raw, page * Program.PAGE_SIZE)
        received[page] = true
    }

    fun missingPages(): List<Int> = (0 until Program.PAGE_COUNT).filter { !received[it] }

    val isComplete: Boolean get() = received.all { it }

    /** Raw page buffer, zeros where pages are missing. */
    fun raw(): ByteArray = raw.copyOf()

    /** The program, or `null` while pages are missing. */
    fun program(): Program? = if (isComplete) Program.fromBytes(raw) else null
}

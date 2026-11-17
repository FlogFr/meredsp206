package dsp206.proto

/**
 * An immutable byte string with value semantics, so frames, requests and
 * names compare by content. Bytes read back as unsigned (0..255).
 */
class Bytes private constructor(private val a: ByteArray) {
    val size: Int get() = a.size

    /** Unsigned byte at [i]. */
    operator fun get(i: Int): Int = a[i].toInt() and 0xff

    /** Little-endian u16 at [i] (protocol.md §2). */
    fun u16(i: Int): Int = get(i) or (get(i + 1) shl 8)

    fun getOrNull(i: Int): Int? = if (i in a.indices) get(i) else null

    fun slice(from: Int, to: Int = size): Bytes = Bytes(a.copyOfRange(from, to))

    fun toByteArray(): ByteArray = a.copyOf()

    fun copyInto(dst: ByteArray, offset: Int) {
        a.copyInto(dst, offset)
    }

    operator fun plus(other: Bytes): Bytes = Bytes(a + other.a)

    fun isEmpty(): Boolean = a.isEmpty()

    fun hex(): String = a.joinToString(" ") { "%02x".format(it.toInt() and 0xff) }

    override fun equals(other: Any?): Boolean = other is Bytes && a.contentEquals(other.a)

    override fun hashCode(): Int = a.contentHashCode()

    override fun toString(): String = "Bytes(${hex()})"

    /** Builds a payload byte by byte. */
    class Builder {
        private val out = java.io.ByteArrayOutputStream()

        fun u8(v: Int): Builder = apply { out.write(v and 0xff) }

        fun u16(v: Int): Builder = apply {
            out.write(v and 0xff)
            out.write((v shr 8) and 0xff)
        }

        fun words(vararg ws: Int): Builder = apply { ws.forEach { u16(it) } }

        fun bytes(b: Bytes): Builder = apply { out.write(b.a) }

        fun build(): Bytes = Bytes(out.toByteArray())
    }

    companion object {
        val EMPTY = Bytes(ByteArray(0))

        fun of(vararg values: Int): Bytes = Bytes(ByteArray(values.size) { values[it].toByte() })

        fun copyOf(array: ByteArray, from: Int = 0, to: Int = array.size): Bytes =
            Bytes(array.copyOfRange(from, to))

        fun zeros(n: Int): Bytes = Bytes(ByteArray(n))

        /** Parse `"10 02 00"` or `"100200"`. */
        fun hex(text: String): Bytes {
            val digits = text.filter { !it.isWhitespace() }
            require(digits.length % 2 == 0) { "odd number of hex digits" }
            return Bytes(ByteArray(digits.length / 2) { digits.substring(2 * it, 2 * it + 2).toInt(16).toByte() })
        }

        fun ascii(text: String): Bytes = Bytes(text.toByteArray(Charsets.US_ASCII))
    }
}

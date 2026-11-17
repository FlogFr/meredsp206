package dsp206.proto

/**
 * Frame encoding and streaming decoding (protocol.md §2).
 *
 * ```
 * 10 02 | src | dst | len | cmd | data[len-1] | 10 03 | xor
 * ```
 *
 * `len` counts the command byte, there is no DLE stuffing, and `xor` covers
 * every preceding byte of the frame.
 */
object Wire {
    const val DLE = 0x10
    const val STX = 0x02
    const val ETX = 0x03

    /** Address of the PC (controller) side. */
    const val PC_ADDRESS = 0x00

    /** Factory default device ID. */
    const val DEFAULT_DEVICE_ID = 1

    /** Bytes around the payload: header (6) + trailer (3), minus the cmd byte counted in `len`. */
    const val OVERHEAD = 8

    /** Largest payload a frame can carry (`len` is one byte and counts `cmd`). */
    const val MAX_DATA = 254

    /** Smallest valid frame (no payload). */
    const val MIN_FRAME = OVERHEAD + 1
}

sealed class FrameError {
    /** The payload does not fit in the one-byte length field. */
    data class PayloadTooLong(val size: Int) : FrameError()

    /** The bytes at `-3..-2` are not `10 03`, or the length does not match. */
    data object BadTrailer : FrameError()

    /** The XOR checksum does not match. */
    data class BadChecksum(val expected: Int, val actual: Int) : FrameError()
}

class FrameException(val error: FrameError) : Exception(error.toString())

/** XOR of `bytes[from until to]`. */
fun checksum(bytes: ByteArray, from: Int = 0, to: Int = bytes.size): Int {
    var x = 0
    for (i in from until to) x = x xor (bytes[i].toInt() and 0xff)
    return x
}

/** One protocol frame, independent of direction. */
data class Frame(val src: Int, val dst: Int, val cmd: Int, val data: Bytes) {
    /** Wire bytes of this frame. */
    fun encode(): ByteArray {
        if (data.size > Wire.MAX_DATA) throw FrameException(FrameError.PayloadTooLong(data.size))
        val n = data.size
        val out = ByteArray(n + Wire.MIN_FRAME)
        out[0] = Wire.DLE.toByte()
        out[1] = Wire.STX.toByte()
        out[2] = src.toByte()
        out[3] = dst.toByte()
        out[4] = (n + 1).toByte()
        out[5] = cmd.toByte()
        data.copyInto(out, 6)
        out[6 + n] = Wire.DLE.toByte()
        out[7 + n] = Wire.ETX.toByte()
        out[8 + n] = checksum(out, 0, 8 + n).toByte()
        return out
    }

    companion object {
        /** A frame sent by the PC to device [dev]. */
        fun request(dev: Int, cmd: Int, data: Bytes = Bytes.EMPTY) = Frame(Wire.PC_ADDRESS, dev, cmd, data)

        /** Parse one complete frame (exactly `len + 8` bytes). */
        fun decode(bytes: ByteArray, offset: Int = 0, length: Int = bytes.size - offset): Frame {
            val n = length
            fun at(i: Int) = bytes[offset + i].toInt() and 0xff
            if (n < Wire.MIN_FRAME || at(0) != Wire.DLE || at(1) != Wire.STX || at(4) + Wire.OVERHEAD != n) {
                throw FrameException(FrameError.BadTrailer)
            }
            if (at(n - 3) != Wire.DLE || at(n - 2) != Wire.ETX) throw FrameException(FrameError.BadTrailer)
            val expected = checksum(bytes, offset, offset + n - 1)
            if (expected != at(n - 1)) throw FrameException(FrameError.BadChecksum(expected, at(n - 1)))
            return Frame(at(2), at(3), at(5), Bytes.copyOf(bytes, offset + 6, offset + n - 3))
        }

        fun decode(bytes: Bytes): Frame = decode(bytes.toByteArray())
    }
}

/** What [FrameDecoder.next] yields: a frame, or a frame that failed validation. */
sealed class Decoded {
    data class Ok(val frame: Frame) : Decoded()

    data class Error(val error: FrameError) : Decoded()
}

/**
 * Incremental decoder for a byte stream (TCP or serial).
 *
 * Bytes before a `10 02` header are skipped. A frame that fails validation is
 * reported once, then the decoder resynchronises on the next header.
 */
class FrameDecoder {
    private var buf = ByteArray(512)
    private var len = 0

    fun push(bytes: ByteArray, offset: Int = 0, count: Int = bytes.size - offset) {
        if (len + count > buf.size) buf = buf.copyOf(maxOf(buf.size * 2, len + count))
        bytes.copyInto(buf, len, offset, offset + count)
        len += count
    }

    /** Bytes buffered but not yet consumed. */
    val pending: Int get() = len

    private fun at(i: Int) = buf[i].toInt() and 0xff

    private fun drain(n: Int) {
        buf.copyInto(buf, 0, n, len)
        len -= n
    }

    private fun sync() {
        var i = 0
        while (i + 1 < len && !(at(i) == Wire.DLE && at(i + 1) == Wire.STX)) i++
        if (i + 1 < len) {
            if (i > 0) drain(i)
        } else {
            // Keep a trailing DLE: it may start the next header.
            val keep = if (len > 0 && at(len - 1) == Wire.DLE) 1 else 0
            drain(len - keep)
        }
    }

    /** The next complete frame or error, or `null` when more bytes are needed. */
    fun next(): Decoded? {
        sync()
        if (len < Wire.MIN_FRAME || at(0) != Wire.DLE || at(1) != Wire.STX) return null
        val total = at(4) + Wire.OVERHEAD
        if (len < total) return null
        return try {
            val frame = Frame.decode(buf, 0, total)
            drain(total)
            Decoded.Ok(frame)
        } catch (e: FrameException) {
            // Drop this header and look for the next one.
            drain(2)
            Decoded.Error(e.error)
        }
    }

    /** Every result available now. */
    fun drainAll(): List<Decoded> = generateSequence { next() }.toList()
}

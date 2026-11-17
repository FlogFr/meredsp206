package dsp206.proto

/**
 * USB HID framing (protocol.md §1.2).
 *
 * The unit enumerates as a generic HID device with 64-byte input and output
 * reports and no report IDs. Each report carries one ordinary frame from its
 * first byte; the rest of the report is padding. Input padding is stale data
 * from earlier frames, so a receiver must cut each report by the frame length.
 */
object Hid {
    /** USB vendor ID ("Musicrown"). */
    const val VENDOR_ID = 0x0168

    /** USB product ID ("Dsp Process"). */
    const val PRODUCT_ID = 0x0821

    /** Size of every input and output report. */
    const val REPORT_LEN = 64

    /** The output report carrying [frame]: the frame bytes, then zero padding. */
    fun outputReport(frame: ByteArray): ByteArray {
        if (frame.size > REPORT_LEN) throw HidException(frame.size)
        return frame.copyOf(REPORT_LEN)
    }

    /**
     * Length of the frame inside an input report of [received] bytes, without
     * the padding. Like the original editor, this trusts the length byte of the
     * frame at the start of the report (`report[4] + 8` bytes) and leaves
     * validation to the frame decoder. A report too short to hold a header is
     * used whole.
     */
    fun inputFrameLength(report: ByteArray, received: Int = report.size): Int {
        if (received < Wire.MIN_FRAME) return received
        val len = (report[4].toInt() and 0xff) + Wire.OVERHEAD
        return minOf(len, received)
    }
}

/** The frame does not fit in one report. */
class HidException(val size: Int) : Exception("frame of $size bytes exceeds the ${Hid.REPORT_LEN}-byte report")

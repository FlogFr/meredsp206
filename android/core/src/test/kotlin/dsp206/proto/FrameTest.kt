package dsp206.proto

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

class FrameTest {
    // Frames captured on a real DSP 206 (protocol.md §2.1).
    private val connectTx = "10 02 00 01 01 10 10 03 11"
    private val connectRx = "10 02 01 00 02 10 19 10 03 0b"
    private val identifyTx = "10 02 00 01 01 13 10 03 12"
    private val identifyRx = "10 02 01 00 0e 13 44 53 50 32 30 36 20 56 30 31 30 35 50 10 03 4c"
    private val page0Tx = "10 02 00 01 02 27 00 10 03 25"

    private fun hex(s: String) = Bytes.hex(s).toByteArray()

    @Test
    fun encodesCapturedRequests() {
        assertEquals(Bytes.hex(connectTx), Bytes.copyOf(Frame.request(1, 0x10).encode()))
        assertEquals(Bytes.hex(identifyTx), Bytes.copyOf(Frame.request(1, 0x13).encode()))
        assertEquals(Bytes.hex(page0Tx), Bytes.copyOf(Frame.request(1, 0x27, Bytes.of(0)).encode()))
    }

    @Test
    fun decodesCapturedReplies() {
        assertEquals(Frame(1, 0, 0x10, Bytes.of(0x19)), Frame.decode(hex(connectRx)))
        val f = Frame.decode(hex(identifyRx))
        assertEquals(0x13, f.cmd)
        assertEquals(Bytes.ascii("DSP206 V0105P"), f.data)
    }

    @Test
    fun roundTripsPayloadContainingDle() {
        // No DLE stuffing: 0x10 bytes in the payload are sent as is.
        val f = Frame.request(1, 0x33, Bytes.of(0x10, 0x02, 0x10, 0x03, 0x10))
        val bytes = f.encode()
        assertEquals(5 + Wire.MIN_FRAME, bytes.size)
        assertEquals(f, Frame.decode(bytes))
    }

    @Test
    fun rejectsBadChecksumAndTrailer() {
        val bad = hex(connectRx).also { it[6] = (it[6].toInt() xor 0xff).toByte() }
        assertIs<FrameError.BadChecksum>(assertFailsWith<FrameException> { Frame.decode(bad) }.error)
        val trailer = hex(connectRx).also { it[7] = 0x11 }
        assertEquals(FrameError.BadTrailer, assertFailsWith<FrameException> { Frame.decode(trailer) }.error)
    }

    @Test
    fun rejectsOversizedPayload() {
        val e = assertFailsWith<FrameException> { Frame.request(1, 0x26, Bytes.zeros(Wire.MAX_DATA + 1)).encode() }
        assertEquals(FrameError.PayloadTooLong(Wire.MAX_DATA + 1), e.error)
        Frame.request(1, 0x26, Bytes.zeros(Wire.MAX_DATA)).encode()
    }

    @Test
    fun decoderHandlesSplitAndConcatenatedFrames() {
        val stream = hex(connectRx) + hex(identifyRx)
        val dec = FrameDecoder()
        val got = mutableListOf<Int>()
        for (b in stream) {
            dec.push(byteArrayOf(b))
            dec.drainAll().forEach { got += (it as Decoded.Ok).frame.cmd }
        }
        assertEquals(listOf(0x10, 0x13), got)
        assertEquals(0, dec.pending)
    }

    @Test
    fun decoderSkipsGarbageAndResyncsAfterCorruption() {
        val bad = hex(connectRx).also { it[6] = (it[6].toInt() xor 1).toByte() }
        val stream = byteArrayOf(-1, 0x00, 0x10, 0x55) + bad + hex(identifyRx)
        val dec = FrameDecoder()
        dec.push(stream)
        val results = dec.drainAll()
        assertIs<FrameError.BadChecksum>((results.first() as Decoded.Error).error)
        assertEquals(0x13, (results.last() as Decoded.Ok).frame.cmd)
    }

    @Test
    fun decoderWaitsForIncompleteFrame() {
        val full = hex(identifyRx)
        val dec = FrameDecoder()
        dec.push(full, 0, 10)
        assertNull(dec.next())
        dec.push(full, 10, full.size - 10)
        assertEquals(0x13, (dec.next() as Decoded.Ok).frame.cmd)
    }

    @Test
    fun decoderKeepsTrailingDle() {
        val dec = FrameDecoder()
        dec.push(byteArrayOf(0x55, 0x10))
        assertNull(dec.next())
        assertEquals(1, dec.pending)
        dec.push(hex(connectRx).copyOfRange(1, 10))
        assertTrue(dec.next() is Decoded.Ok)
    }
}

class HidTest {
    @Test
    fun outputReportIsFrameThenZeroPadding() {
        val frame = Bytes.hex("10 02 00 01 01 10 10 03 11").toByteArray() // connect (protocol.md §2.1)
        val report = Hid.outputReport(frame)
        assertEquals(64, report.size)
        assertEquals(Bytes.copyOf(frame), Bytes.copyOf(report, 0, 9))
        assertTrue(report.drop(9).all { it == 0.toByte() })
    }

    @Test
    fun outputReportRejectsFramesLongerThanAReport() {
        val fits = Frame.request(1, 0x25, Bytes.zeros(Hid.REPORT_LEN - Wire.OVERHEAD - 1)).encode()
        assertEquals(Hid.REPORT_LEN, fits.size)
        Hid.outputReport(fits)
        val long = Frame.request(1, 0x25, Bytes.zeros(Hid.REPORT_LEN - Wire.OVERHEAD)).encode()
        assertEquals(65, assertFailsWith<HidException> { Hid.outputReport(long) }.size)
    }

    @Test
    fun inputFrameIsCutByLength() {
        // Identify reply padded with stale bytes that look like another frame.
        val reply = Bytes.hex("10 02 01 00 0e 13 44 53 50 32 30 36 20 56 30 31 30 35 50 10 03 4c").toByteArray()
        val report = (reply + byteArrayOf(0x10, 0x02, 0x01, 0x00, 0x34)).copyOf(Hid.REPORT_LEN)
        report.fill(0xaa.toByte(), reply.size + 5, Hid.REPORT_LEN)
        assertEquals(reply.size, Hid.inputFrameLength(report))
        val dec = FrameDecoder()
        dec.push(report, 0, Hid.inputFrameLength(report))
        assertEquals(Bytes.ascii("DSP206 V0105P"), (dec.next() as Decoded.Ok).frame.data)
        assertEquals(0, dec.pending)
    }

    @Test
    fun inputFrameNeverReadsPastTheReport() {
        val report = ByteArray(Hid.REPORT_LEN)
        byteArrayOf(0x10, 0x02, 0x01, 0x00, -1).copyInto(report)
        assertEquals(Hid.REPORT_LEN, Hid.inputFrameLength(report))
        assertEquals(2, Hid.inputFrameLength(byteArrayOf(0x10, 0x02)))
        assertEquals(Hid.VENDOR_ID, 0x0168)
        assertEquals(Hid.PRODUCT_ID, 0x0821)
    }
}

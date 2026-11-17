package dsp206.link

import dsp206.proto.Hid
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketTimeoutException
import javax.net.SocketFactory

/** The byte pipe under a [Link]: TCP, USB HID or the simulated device. */
interface Transport {
    /** Send one complete frame. */
    @Throws(IOException::class)
    fun send(frame: ByteArray)

    /**
     * Wait a few milliseconds for incoming bytes. Returns the number of bytes
     * stored in [buf], `0` when nothing arrived, or `-1` when the connection is
     * gone.
     */
    @Throws(IOException::class)
    fun recv(buf: ByteArray): Int

    fun close()
}

/** TCP port of the Ethernet module (Microchip UART-to-TCP bridge, protocol.md §1.1). */
const val TCP_PORT = 9761

/** The Ethernet module over TCP. The socket has a short read timeout so the worker can tick. */
class TcpTransport(private val socket: Socket) : Transport {
    private val input = socket.getInputStream()
    private val output = socket.getOutputStream()

    init {
        socket.tcpNoDelay = true
        socket.soTimeout = POLL_MS
    }

    override fun send(frame: ByteArray) {
        output.write(frame)
        output.flush()
    }

    override fun recv(buf: ByteArray): Int = try {
        input.read(buf)
    } catch (_: SocketTimeoutException) {
        0
    }

    override fun close() {
        try {
            socket.close()
        } catch (_: IOException) {
        }
    }

    companion object {
        const val POLL_MS = 5

        /**
         * Open the connection (blocking). On Android, pass the Wi-Fi network's
         * socket factory so the socket does not go out over mobile data.
         */
        fun connect(address: InetSocketAddress, timeoutMs: Int, factory: SocketFactory = SocketFactory.getDefault()): TcpTransport {
            val socket = factory.createSocket()
            try {
                socket.connect(address, timeoutMs)
                return TcpTransport(socket)
            } catch (e: IOException) {
                socket.close()
                throw e
            }
        }
    }
}

/** Raw 64-byte HID reports, as the Android USB host API or a test bridge provides them. */
interface HidPort {
    /** Write one 64-byte output report (no report ID: the unit does not number its reports). */
    @Throws(IOException::class)
    fun write(report: ByteArray)

    /** Read one input report into [report], waiting at most [timeoutMs]. Returns the byte count, 0 when none arrived. */
    @Throws(IOException::class)
    fun read(report: ByteArray, timeoutMs: Int): Int

    fun close()
}

/**
 * USB HID transport (protocol.md §1.2): one frame per 64-byte report, zero
 * padded on output. Input reports are cut by the frame length because their
 * padding holds stale bytes of earlier frames.
 */
class HidTransport(private val port: HidPort, private val pollMs: Int = 5) : Transport {
    private val report = ByteArray(Hid.REPORT_LEN)

    override fun send(frame: ByteArray) = port.write(Hid.outputReport(frame))

    override fun recv(buf: ByteArray): Int {
        val n = port.read(report, pollMs)
        if (n <= 0) return 0
        val len = Hid.inputFrameLength(report, n)
        report.copyInto(buf, 0, 0, minOf(len, buf.size))
        return len
    }

    override fun close() = port.close()
}

package dsp206.link

import java.io.IOException
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException

/** Microchip "announce" discovery on UDP 30303 (protocol.md §1.1). */
object Discovery {
    const val PORT = 30303
    val PROBE: ByteArray = "Discovery: Who is out there?\u0000".toByteArray(Charsets.US_ASCII)

    data class Found(
        /** The reply's source address: the device IP (connection.md §2). */
        val address: InetAddress,
        /** First line of the reply (the module's host name, e.g. `EHTERNET_COM`). */
        val name: String,
        /** Remaining non-empty lines (partial MAC address, …). */
        val details: List<String>,
    )

    /** Parse one announce reply. */
    fun parseReply(address: InetAddress, data: ByteArray, length: Int = data.size): Found {
        val lines = String(data, 0, length, Charsets.ISO_8859_1)
            .split('\r', '\n')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        return Found(address, lines.firstOrNull().orEmpty(), lines.drop(1))
    }

    /** The IPv4 broadcast address of a subnet, e.g. `192.168.1.255` for `192.168.1.20/24`. */
    fun broadcastAddress(address: Inet4Address, prefixLength: Int): Inet4Address {
        require(prefixLength in 0..32)
        val a = address.address
        val ip = ((a[0].toInt() and 0xff) shl 24) or ((a[1].toInt() and 0xff) shl 16) or
            ((a[2].toInt() and 0xff) shl 8) or (a[3].toInt() and 0xff)
        val hostMask = if (prefixLength == 0) -1 else (1 shl (32 - prefixLength)) - 1
        val b = ip or hostMask
        val bytes = byteArrayOf((b shr 24).toByte(), (b shr 16).toByte(), (b shr 8).toByte(), b.toByte())
        return InetAddress.getByAddress(bytes) as Inet4Address
    }

    /**
     * Send the probe to every target (the limited broadcast address
     * `255.255.255.255` and, ideally, the Wi-Fi subnet's broadcast address) and
     * collect replies for [waitMs]. [prepare] can bind the socket to a network
     * before anything is sent. Nothing but this probe is ever sent.
     */
    fun discover(
        targets: List<InetAddress>,
        waitMs: Long = 1500,
        port: Int = PORT,
        prepare: (DatagramSocket) -> Unit = {},
    ): List<Found> {
        val found = mutableListOf<Found>()
        DatagramSocket(null as java.net.SocketAddress?).use { sock ->
            sock.reuseAddress = true
            sock.bind(InetSocketAddress(0))
            prepare(sock)
            sock.broadcast = true
            sock.soTimeout = 50
            for (t in targets.distinct()) {
                // A failing target (no route) must not stop the others.
                try {
                    sock.send(DatagramPacket(PROBE, PROBE.size, InetSocketAddress(t, port)))
                } catch (_: IOException) {
                }
            }
            val end = System.nanoTime() + waitMs * 1_000_000
            val buf = ByteArray(512)
            while (System.nanoTime() < end) {
                val packet = DatagramPacket(buf, buf.size)
                try {
                    sock.receive(packet)
                } catch (_: SocketTimeoutException) {
                    continue
                }
                val data = buf.copyOf(packet.length)
                if (data.contentEquals(PROBE)) continue // our own broadcast looped back
                val d = parseReply(packet.address, data)
                if (found.none { it.address == d.address }) found += d
            }
        }
        return found
    }

    val LIMITED_BROADCAST: InetAddress = InetAddress.getByAddress(byteArrayOf(-1, -1, -1, -1))
}

package dsp206.link

import java.net.Inet4Address
import java.net.InetAddress
import kotlin.test.Test
import kotlin.test.assertEquals

class DiscoveryTest {
    private fun ip(s: String) = InetAddress.getByName(s) as Inet4Address

    @Test
    fun parsesCapturedAnnounce() {
        // Reply captured from the real module (protocol.md §1.1).
        val d = Discovery.parseReply(ip("192.168.1.101"), "EHTERNET_COM   \r\n00-04-A3-C1\r\n1F\r\n".toByteArray())
        assertEquals(ip("192.168.1.101"), d.address)
        assertEquals("EHTERNET_COM", d.name)
        assertEquals(listOf("00-04-A3-C1", "1F"), d.details)
    }

    @Test
    fun probeIsTheMicrochipAnnounceString() {
        assertEquals("Discovery: Who is out there?", String(Discovery.PROBE, 0, Discovery.PROBE.size - 1))
        assertEquals(0, Discovery.PROBE.last().toInt())
        assertEquals(30303, Discovery.PORT)
        assertEquals(9761, TCP_PORT)
    }

    @Test
    fun broadcastAddresses() {
        assertEquals(ip("192.168.1.255"), Discovery.broadcastAddress(ip("192.168.1.20"), 24))
        assertEquals(ip("10.0.255.255"), Discovery.broadcastAddress(ip("10.0.3.4"), 16))
        assertEquals(ip("192.168.1.127"), Discovery.broadcastAddress(ip("192.168.1.101"), 25))
        assertEquals(ip("192.168.1.101"), Discovery.broadcastAddress(ip("192.168.1.101"), 32))
        assertEquals(ip("255.255.255.255"), Discovery.broadcastAddress(ip("192.168.1.101"), 0))
        assertEquals(ip("255.255.255.255"), Discovery.LIMITED_BROADCAST)
    }

    @Test
    fun discoveryFindsALocalResponder() {
        // A fake module on localhost answers the probe from another port.
        java.net.DatagramSocket(0, InetAddress.getLoopbackAddress()).use { module ->
            val t = kotlin.concurrent.thread {
                val buf = ByteArray(64)
                val p = java.net.DatagramPacket(buf, buf.size)
                module.receive(p)
                val reply = "EHTERNET_COM   \r\n00-04-A3-C1\r\n".toByteArray()
                module.send(java.net.DatagramPacket(reply, reply.size, p.socketAddress))
            }
            val found = Discovery.discover(listOf(InetAddress.getLoopbackAddress()), waitMs = 500, port = module.localPort)
            t.join(1000)
            assertEquals(listOf("EHTERNET_COM"), found.map { it.name })
            assertEquals(listOf(listOf("00-04-A3-C1")), found.map { it.details })
            assertEquals(InetAddress.getLoopbackAddress(), found.single().address)
        }
    }
}

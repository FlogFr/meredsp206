package io.github.flogfr.meredsp206.device

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import dsp206.link.Discovery
import java.net.Inet4Address
import java.net.InetAddress

/** The phone's address on the local network, e.g. 192.168.1.23/24. */
data class LocalAddress(val address: Inet4Address, val prefixLength: Int) {
    val broadcast: Inet4Address get() = Discovery.broadcastAddress(address, prefixLength)

    override fun toString() = "${address.hostAddress}/$prefixLength"
}

/**
 * Follows the Wi-Fi (or Ethernet) network the unit is reachable on. Sockets
 * to the unit are created on that network: when the Wi-Fi has no internet
 * access, Android would otherwise send them over mobile data.
 */
class LocalNetwork(context: Context, private val onChange: () -> Unit) {
    private val cm = context.getSystemService(ConnectivityManager::class.java)

    @Volatile
    var network: Network? = null
        private set

    @Volatile
    var address: LocalAddress? = null
        private set

    private val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(n: Network) {
            network = n
            cm?.getLinkProperties(n)?.let { update(it) }
            onChange()
        }

        override fun onLinkPropertiesChanged(n: Network, lp: LinkProperties) {
            if (n == network) {
                update(lp)
                onChange()
            }
        }

        override fun onLost(n: Network) {
            if (n == network) {
                network = null
                address = null
                onChange()
            }
        }
    }

    private fun update(lp: LinkProperties) {
        address = lp.linkAddresses
            .firstOrNull { it.address is Inet4Address }
            ?.let { LocalAddress(it.address as Inet4Address, it.prefixLength) }
    }

    fun start() {
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .addTransportType(NetworkCapabilities.TRANSPORT_ETHERNET)
            // A Wi-Fi router without internet access still reaches the unit.
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        cm?.registerNetworkCallback(request, callback)
    }

    fun stop() {
        try {
            cm?.unregisterNetworkCallback(callback)
        } catch (_: IllegalArgumentException) {
            // Not registered.
        }
    }

    /** Where the discovery probe goes: the limited broadcast and the subnet's broadcast address. */
    fun discoveryTargets(): List<InetAddress> = listOfNotNull(Discovery.LIMITED_BROADCAST, address?.broadcast)
}

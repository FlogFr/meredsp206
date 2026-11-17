package io.github.flogfr.meredsp206

import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.hardware.usb.UsbManager
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dsp206.editor.LinkKind
import dsp206.editor.Session
import dsp206.editor.UiNotice
import dsp206.link.Discovery
import dsp206.link.Event
import dsp206.link.HidTransport
import dsp206.link.Link
import dsp206.link.LinkConfig
import dsp206.link.MockDevice
import dsp206.link.MockOptions
import dsp206.link.MockTransport
import dsp206.link.TCP_PORT
import dsp206.link.TcpTransport
import dsp206.link.Transport
import dsp206.proto.Channel
import dsp206.proto.Meters
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.linkRequests
import dsp206.proto.programFromFile
import dsp206.proto.syncRequests
import dsp206.proto.toFile
import io.github.flogfr.meredsp206.device.LocalAddress
import io.github.flogfr.meredsp206.device.LocalNetwork
import io.github.flogfr.meredsp206.device.UsbConnector
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel as NoticeChannel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException
import java.net.InetSocketAddress
import javax.net.SocketFactory

/** Messages for the user, shown once in a snackbar. */
sealed class AppNotice {
    data class FromSession(val notice: UiNotice) : AppNotice()

    data class Connected(val kind: LinkKind) : AppNotice()

    data class ConnectFailed(val detail: String?) : AppNotice()

    data class NotAnAddress(val text: String) : AppNotice()

    data class ReconnectTooSoon(val seconds: Int) : AppNotice()

    data object NoUsbDevice : AppNotice()

    data object UsbPermissionDenied : AppNotice()

    data object NotConnected : AppNotice()

    data class Recalling(val n: Int) : AppNotice()

    data class Storing(val n: Int, val name: String) : AppNotice()

    data class Sending(val count: Int) : AppNotice()

    data object FileSaved : AppNotice()

    data object FileInvalid : AppNotice()

    data class Copied(val src: Channel, val dst: Channel) : AppNotice()
}

/** Connection facts shown on the device page and in the top bar. */
data class LinkStatus(
    val kind: LinkKind = LinkKind.OFFLINE,
    val target: String = "",
    val healthSilent: Boolean = false,
    val unconfirmed: Int = 0,
    val timeouts: Int = 0,
    val missingPages: List<Int> = emptyList(),
    val pagesReceived: Int = 0,
    val reconnectWait: Int? = null,
)

/** What the unit reported, plus undo state. */
data class DeviceInfo(
    val identity: String? = null,
    val currentPreset: Int? = null,
    val presetNames: List<String> = emptyList(),
    val usedSlots: Set<Int> = emptySet(),
    val canUndo: Boolean = false,
    val canRedo: Boolean = false,
)

/**
 * The app state: one [Link] at a time, the [Session] it feeds, and the
 * Compose state the screens read. All state changes happen on the main
 * thread; link events are posted to it.
 */
class DspViewModel(app: Application) : AndroidViewModel(app) {
    private val session = Session()
    private val main = Handler(Looper.getMainLooper())
    private val prefs = app.getSharedPreferences("meredsp206", Context.MODE_PRIVATE)
    private val usb = UsbConnector(app)
    private val localNetwork = LocalNetwork(app) { main.post { networkChanged() } }
    private var link: Link? = null

    /** Events of older links are ignored. */
    private var generation = 0
    private var countdown: Job? = null
    private var foreground = false
    private var meterView = false

    var program: Program by mutableStateOf(session.model.program)
        private set
    var info by mutableStateOf(DeviceInfo())
        private set
    var status by mutableStateOf(LinkStatus())
        private set
    var meters: Meters? by mutableStateOf(null)
        private set
    var logVersion by mutableIntStateOf(0)
        private set
    var localAddress: LocalAddress? by mutableStateOf(null)
        private set
    var usbPresent by mutableStateOf(false)
        private set
    var discovering by mutableStateOf(false)
        private set
    var found: List<Discovery.Found> by mutableStateOf(emptyList())
        private set

    var address by mutableStateOf(prefs.getString(KEY_ADDRESS, null) ?: dsp206.editor.Ipv4.FACTORY_ADDRESS)
        private set
    var deviceId by mutableIntStateOf(prefs.getInt(KEY_DEVICE_ID, 1))
        private set
    var selected by mutableStateOf(Channel.IN_A)
        private set
    var selectedBand by mutableIntStateOf(0)
        private set

    private val noticeChannel = NoticeChannel<AppNotice>(NoticeChannel.BUFFERED)
    val notices: Flow<AppNotice> = noticeChannel.receiveAsFlow()

    private val usbReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (!UsbConnector.isDsp206(intent)) return
            val attached = intent.action == UsbManager.ACTION_USB_DEVICE_ATTACHED
            if (!attached && status.kind == LinkKind.USB) disconnect()
            refreshUsb()
        }
    }

    init {
        localNetwork.start()
        ContextCompat.registerReceiver(app, usbReceiver, UsbConnector.attachFilter(), ContextCompat.RECEIVER_NOT_EXPORTED)
        refreshUsb()
    }

    override fun onCleared() {
        disconnect()
        localNetwork.stop()
        getApplication<Application>().unregisterReceiver(usbReceiver)
    }

    private fun notify(n: AppNotice) {
        noticeChannel.trySend(n)
    }

    private fun networkChanged() {
        localAddress = localNetwork.address
    }

    fun refreshUsb() {
        usbPresent = usb.find() != null
    }

    // ---- connection ---------------------------------------------------------

    fun updateAddress(text: String) {
        address = text
    }

    fun updateDeviceId(id: Int) {
        deviceId = id.coerceIn(1, 254)
        prefs.edit { putInt(KEY_DEVICE_ID, deviceId) }
    }

    fun connectNetwork(text: String = address) {
        val ip = dsp206.editor.Ipv4.parse(text) ?: return notify(AppNotice.NotAnAddress(text))
        address = ip.hostAddress ?: text
        prefs.edit { putString(KEY_ADDRESS, address) }
        session.reconnectWaitS()?.let { notify(AppNotice.ReconnectTooSoon(it)) }
        disconnect()
        val gen = ++generation
        session.connecting()
        refresh(target = address)
        val factory = localNetwork.network?.socketFactory ?: SocketFactory.getDefault()
        val cfg = LinkConfig(deviceId = deviceId)
        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    Result.success(TcpTransport.connect(InetSocketAddress(ip, TCP_PORT), cfg.connectTimeoutMs, factory))
                } catch (e: IOException) {
                    Result.failure(e)
                }
            }
            if (gen != generation) {
                result.getOrNull()?.close()
                return@launch
            }
            result.fold(
                onSuccess = { start(it, LinkKind.NETWORK, address, cfg) },
                onFailure = {
                    session.connectFailed(network = true)
                    refresh()
                    notify(AppNotice.ConnectFailed(it.message))
                    startCountdown()
                },
            )
        }
    }

    fun connectUsb() {
        val device = usb.find() ?: return notify(AppNotice.NoUsbDevice)
        if (!usb.hasPermission(device)) {
            usb.requestPermission(device) { granted ->
                if (granted) connectUsb() else notify(AppNotice.UsbPermissionDenied)
            }
            return
        }
        disconnect()
        val port = try {
            usb.open(device)
        } catch (e: IOException) {
            return notify(AppNotice.ConnectFailed(e.message))
        }
        ++generation
        start(HidTransport(port), LinkKind.USB, "USB", LinkConfig(deviceId = deviceId))
    }

    fun startDemo() {
        disconnect()
        ++generation
        // The real unit answers USB requests in about 10 ms (protocol.md §1.2).
        val device = MockDevice(MockOptions(latencyMs = 10, presetDelayMs = 1500))
        start(MockTransport(device), LinkKind.DEMO, "", LinkConfig())
    }

    private fun start(transport: Transport, kind: LinkKind, target: String, cfg: LinkConfig) {
        val gen = generation
        val l = Link.start(transport, cfg) { ev -> main.post { if (gen == generation) onEvent(ev) } }
        link = l
        session.connected(kind)
        l.readAll()
        applyMetering()
        refresh(target)
        notify(AppNotice.Connected(kind))
    }

    /** Close the link, or cancel a connection attempt. */
    fun disconnect() {
        val l = link
        if (l == null && session.kind != LinkKind.CONNECTING) return
        link = null
        generation++
        l?.close()
        session.closed()
        meters = null
        refresh()
        startCountdown()
    }

    /** Count down the pause the network module needs before a new session (connection.md §2). */
    private fun startCountdown() {
        countdown?.cancel()
        countdown = viewModelScope.launch {
            while (isActive && session.reconnectWaitS() != null) {
                status = status.copy(reconnectWait = session.reconnectWaitS())
                delay(1000)
            }
            status = status.copy(reconnectWait = null)
        }
    }

    fun readAgain() {
        val l = link ?: return notify(AppNotice.NotConnected)
        session.programReadStarted()
        l.readAll()
        refresh()
    }

    fun retryMissingPages() {
        val l = link ?: return notify(AppNotice.NotConnected)
        l.retryMissingPages()
    }

    // ---- metering: only while a view with meters is on screen ---------------

    fun setForeground(on: Boolean) {
        foreground = on
        applyMetering()
    }

    fun setMeterView(on: Boolean) {
        meterView = on
        applyMetering()
    }

    private fun applyMetering() {
        link?.setMetering(foreground && meterView)
        if (!(foreground && meterView)) meters = null
    }

    // ---- editing --------------------------------------------------------------

    fun select(ch: Channel) {
        if (ch != selected) {
            selected = ch
            selectedBand = 0
        }
    }

    fun selectBand(band: Int) {
        selectedBand = band
    }

    /** A user edit: applied to the local program, then sent when it changed something. */
    fun edit(req: Request) {
        val sent = session.edit(req) ?: return
        link?.send(sent)
        refresh()
    }

    fun setLinked(ch: Channel, other: Channel, join: Boolean) {
        program.linkRequests(ch, other, join).forEach { edit(it) }
    }

    fun copyChannel(src: Channel, dst: Channel) {
        edit(Request.CopyChannel(src, dst))
        notify(AppNotice.Copied(src, dst))
    }

    fun undo() {
        session.model.undo().forEach { link?.send(it) }
        refresh()
    }

    fun redo() {
        session.model.redo().forEach { link?.send(it) }
        refresh()
    }

    // ---- presets and files -------------------------------------------------------

    fun recallPreset(n: Int) {
        val l = link ?: return notify(AppNotice.NotConnected)
        // The worker keeps the order: recall first, then re-read the program.
        l.send(Request.RecallPreset(n))
        session.programReadStarted()
        l.readProgram()
        l.send(Request.GetCurrentPreset)
        notify(AppNotice.Recalling(n))
    }

    fun storePreset(n: Int, name: String) {
        val l = link ?: return notify(AppNotice.NotConnected)
        session.model.beginStore(n, name).forEach { l.send(it) }
        notify(AppNotice.Storing(n, name.trim()))
        refresh()
    }

    /** Send every setting of the local program to the unit. */
    fun pushProgram() {
        val l = link ?: return notify(AppNotice.NotConnected)
        val reqs = program.syncRequests()
        reqs.forEach { l.send(it) }
        notify(AppNotice.Sending(reqs.size))
    }

    fun programFile(): ByteArray = program.toFile()

    fun fileSaved() = notify(AppNotice.FileSaved)

    /** Load a program file: the differences are sent to the unit when connected. */
    fun loadProgramFile(bytes: ByteArray) {
        val p = try {
            programFromFile(bytes)
        } catch (_: dsp206.proto.ProgramException) {
            return notify(AppNotice.FileInvalid)
        }
        val reqs = session.model.load(p)
        link?.let { l -> reqs.forEach { l.send(it) } }
        refresh()
        notify(AppNotice.Sending(reqs.size))
    }

    // ---- events and state ---------------------------------------------------------

    fun log(): List<String> = session.log

    fun clearLog() {
        session.clearLog()
        logVersion++
    }

    private fun onEvent(ev: Event) {
        val notice = session.handle(ev)
        when (ev) {
            is Event.Trace -> logVersion++
            is Event.MeterLevels -> meters = if (foreground && meterView) session.meters else null
            else -> {
                if (ev is Event.Closed) {
                    link = null
                    meters = null
                    startCountdown()
                }
                refresh()
            }
        }
        notice?.let { notify(AppNotice.FromSession(it)) }
    }

    private fun refresh(target: String = status.target) {
        val m = session.model
        program = m.program
        val slots = session.presetSlots
        info = DeviceInfo(
            identity = m.identity,
            currentPreset = m.currentPreset,
            presetNames = m.presetNames.toList(),
            usedSlots = if (slots == null) emptySet() else (0 until slots.size).filter { slots[it] == 0xff }.map { it + 1 }.toSet(),
            canUndo = m.canUndo,
            canRedo = m.canRedo,
        )
        status = LinkStatus(
            kind = session.kind,
            target = target,
            healthSilent = session.health == dsp206.link.Health.SILENT,
            unconfirmed = session.unconfirmed,
            timeouts = session.timeouts,
            missingPages = session.missingPages,
            pagesReceived = session.pagesReceived,
            reconnectWait = session.reconnectWaitS(),
        )
    }

    // ---- discovery ------------------------------------------------------------------

    /** Broadcast the announce probe on UDP 30303 and list the units that answer. */
    fun discover() {
        if (discovering) return
        discovering = true
        val targets = localNetwork.discoveryTargets()
        val net = localNetwork.network
        viewModelScope.launch {
            found = withContext(Dispatchers.IO) {
                try {
                    Discovery.discover(targets, waitMs = 1500) { sock -> net?.bindSocket(sock) }
                } catch (_: IOException) {
                    emptyList()
                }
            }
            discovering = false
        }
    }

    private companion object {
        const val KEY_ADDRESS = "address"
        const val KEY_DEVICE_ID = "device_id"
    }
}

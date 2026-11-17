package dsp206.editor

import dsp206.link.CloseReason
import dsp206.link.Event
import dsp206.link.Health
import dsp206.proto.Bytes
import dsp206.proto.Meters
import dsp206.proto.Request
import java.net.Inet4Address
import java.net.InetAddress
import java.util.Locale

/** How the app is connected. */
enum class LinkKind {
    OFFLINE,
    CONNECTING,

    /** A real unit over the network (Wi-Fi on the phone, Ethernet on the unit). */
    NETWORK,

    /** A real unit over USB (OTG cable). */
    USB,

    /** The simulated unit. */
    DEMO,
    ;

    val isOnline: Boolean get() = this == NETWORK || this == USB || this == DEMO
}

/** Something the UI tells the user once (a snackbar). */
sealed class UiNotice {
    data class Preset(val notice: Notice) : UiNotice()

    data object ProgramRead : UiNotice()

    data class ProgramIncomplete(val missing: Int) : UiNotice()

    /** The device stopped answering; changes are still sent but not confirmed. */
    data object DeviceSilent : UiNotice()

    data class Disconnected(val reason: CloseReason, val detail: String?) : UiNotice()
}

/**
 * Everything the app knows about the connected unit, updated from link
 * events. Port of the event handling of crates/dsp206-gtk/src/app.rs
 * (`App::poll`). Use it from one thread.
 */
class Session(private val clockMs: () -> Long = { System.nanoTime() / 1_000_000 }) {
    val model = EditorModel()
    var kind = LinkKind.OFFLINE
        private set
    var health = Health.ANSWERING
        private set
    var unconfirmed = 0
        private set
    var timeouts = 0
        private set
    var missingPages: List<Int> = emptyList()
        private set
    var meters: Meters? = null
        private set

    /** Pages received since the last program read started. */
    var pagesReceived = 0
        private set
    var presetSlots: Bytes? = null
        private set

    /** When the last network session ended (the module needs about a minute, connection.md §2). */
    private var lastNetworkClose: Long? = null
    private val started = clockMs()
    private val logLines = ArrayDeque<String>()

    /** Protocol log, oldest first. */
    val log: List<String> get() = logLines.toList()

    fun clearLog() = logLines.clear()

    /** Seconds to wait before reconnecting to a real unit over the network is advisable. */
    fun reconnectWaitS(): Int? {
        val since = clockMs() - (lastNetworkClose ?: return null)
        return if (since < RECONNECT_DELAY_MS) ((RECONNECT_DELAY_MS - since + 999) / 1000).toInt() else null
    }

    fun connecting() {
        kind = LinkKind.CONNECTING
    }

    /** A link is up; the full read starts. */
    fun connected(kind: LinkKind) {
        this.kind = kind
        health = Health.ANSWERING
        unconfirmed = 0
        timeouts = 0
        missingPages = emptyList()
        pagesReceived = 0
    }

    /** The link ended, by [disconnect] or because of an error. */
    fun closed() {
        if (kind == LinkKind.NETWORK || kind == LinkKind.CONNECTING) lastNetworkClose = clockMs()
        kind = LinkKind.OFFLINE
        meters = null
    }

    /** A connection attempt failed before any link existed. */
    fun connectFailed(network: Boolean) {
        if (network) lastNetworkClose = clockMs()
        kind = LinkKind.OFFLINE
    }

    fun programReadStarted() {
        pagesReceived = 0
    }

    /** A user edit: returns the setter to send, or null when nothing changed. */
    fun edit(req: Request): Request? = model.edit(req, clockMs())

    /** Apply one link event. Returns what to tell the user, if anything. */
    fun handle(ev: Event): UiNotice? {
        when (ev) {
            is Event.Trace -> trace(ev.outgoing, ev.bytes)
            is Event.Handshake -> {}
            is Event.Identity -> model.identity = ev.text
            is Event.CurrentPreset -> model.currentPreset = ev.n
            is Event.PresetName -> {
                val outcome = model.storeOutcome(ev.index, ev.name)
                model.setPresetName(ev.index, ev.name)
                return outcome?.let { UiNotice.Preset(it) }
            }
            is Event.PresetSlots -> presetSlots = ev.slots
            is Event.LockState -> {}
            is Event.Page -> {
                model.applyPage(ev.page, ev.data)
                pagesReceived++
            }
            is Event.ProgramRead -> {
                model.setFromDevice(ev.program)
                missingPages = emptyList()
                return UiNotice.ProgramRead
            }
            is Event.ProgramIncomplete -> {
                missingPages = ev.missing
                return UiNotice.ProgramIncomplete(ev.missing.size)
            }
            is Event.MeterLevels -> meters = ev.meters
            is Event.Unconfirmed -> {
                unconfirmed++
                return model.unconfirmed(ev.request)?.let { UiNotice.Preset(it) }
            }
            is Event.Timeout -> timeouts++
            is Event.HealthChanged -> {
                health = ev.health
                if (ev.health == Health.SILENT) return UiNotice.DeviceSilent
            }
            is Event.Closed -> {
                closed()
                return UiNotice.Disconnected(ev.reason, ev.detail)
            }
        }
        return null
    }

    private fun trace(outgoing: Boolean, bytes: Bytes) {
        val t = (clockMs() - started) / 1000.0
        val what = if (bytes.size > 5) Format.command(bytes[5], outgoing) else ""
        val arrow = if (outgoing) "→" else "←"
        logLines.addLast(String.format(Locale.ROOT, "%9.3f  %s  %-44s %s", t, arrow, bytes.hex(), what).trimEnd())
        while (logLines.size > LOG_LINES) logLines.removeFirst()
    }

    companion object {
        /** The Ethernet module needs time before it serves a new session (connection.md §2). */
        const val RECONNECT_DELAY_MS = 60_000L
        const val LOG_LINES = 3000
    }
}

/** IPv4 literals typed by the user. Never resolves names (no DNS on the UI thread). */
object Ipv4 {
    private val DOTTED = Regex("""(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})""")

    fun parse(text: String): Inet4Address? {
        val m = DOTTED.matchEntire(text.trim()) ?: return null
        val parts = m.groupValues.drop(1).map { it.toInt() }
        if (parts.any { it > 255 }) return null
        return InetAddress.getByAddress(ByteArray(4) { parts[it].toByte() }) as Inet4Address
    }

    /** The factory address of the unit's Ethernet module (protocol.md §1.1). */
    const val FACTORY_ADDRESS = "192.168.1.101"
}

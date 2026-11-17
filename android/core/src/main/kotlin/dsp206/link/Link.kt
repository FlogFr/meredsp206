package dsp206.link

import dsp206.proto.Bytes
import dsp206.proto.Meters
import dsp206.proto.Names
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.Wire
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.atomic.AtomicInteger

/*
 * Talking to a DSP 206 over TCP or USB (claude/docs/connection.md). Port of
 * crates/dsp206-link: a Link owns the single connection and a worker thread
 * that keeps exactly one request in flight, applies timeouts and retries,
 * coalesces queued setters and turns replies into Events.
 */

data class LinkConfig(
    val deviceId: Int = Wire.DEFAULT_DEVICE_ID,
    val connectTimeoutMs: Int = 3000,
    /** Reply timeout for reads (original app: 800 ms). */
    val readTimeoutMs: Long = 800,
    /** How long to wait for the `0x01` acknowledgement of a setter. */
    val writeTimeoutMs: Long = 500,
    /** Acknowledgement wait for a preset store `0x21` (original app: 10 s). */
    val storeTimeoutMs: Long = 10_000,
    /** Acknowledgement wait for a preset recall `0x20` (original app: 50 s). */
    val recallTimeoutMs: Long = 50_000,
    /** How many times an unanswered request is sent again (original app: 2). */
    val retries: Int = 2,
    /** Quiet time after each reply or abandoned request before the next frame. */
    val gapMs: Long = 0,
    /** Level-meter poll period when metering is on (original app: 300 ms). */
    val meterIntervalMs: Long = 300,
    /** Consecutive unanswered requests before the link is reported silent. */
    val silentAfter: Int = 3,
)

/** Whether the device is answering. */
enum class Health {
    ANSWERING,

    /** Several requests in a row got no reply (connection.md §1). */
    SILENT,
}

/** Why a link ended. */
enum class CloseReason {
    /** [Link.close] was called. */
    CLOSED,

    /** The device or the network closed the connection. */
    REMOTE_CLOSED,
    SEND_FAILED,
    RECEIVE_FAILED,
}

/** What the worker reports, on the worker thread. */
sealed class Event {
    data class Handshake(val status: Int) : Event()

    data class Identity(val text: String) : Event()

    data class CurrentPreset(val n: Int) : Event()

    data class PresetSlots(val slots: Bytes) : Event()

    /** [index] is the preset number (1 = U01); 21 is the current program. */
    data class PresetName(val index: Int, val name: String) : Event()

    data class LockState(val state: Bytes) : Event()

    /** One program page arrived ([data] excludes the page number). */
    data class Page(val page: Int, val data: Bytes) : Event()

    /** All 19 pages of a program read arrived. */
    data class ProgramRead(val program: Program) : Event()

    /** A program read finished with these pages missing. */
    data class ProgramIncomplete(val missing: List<Int>) : Event()

    data class MeterLevels(val meters: Meters) : Event()

    /** A setter was sent but no acknowledgement came back. */
    data class Unconfirmed(val request: Request) : Event()

    /** A read got no reply after all retries. */
    data class Timeout(val request: Request) : Event()

    data class HealthChanged(val health: Health) : Event()

    /** Every frame sent ([outgoing]) or received, for the protocol log. */
    data class Trace(val outgoing: Boolean, val bytes: Bytes) : Event()

    /** The connection ended; the worker has stopped. */
    data class Closed(val reason: CloseReason, val detail: String? = null) : Event()
}

internal sealed class Cmd {
    data class Send(val req: Request) : Cmd()

    data object ReadAll : Cmd()

    data class ReadProgram(val fresh: Boolean) : Cmd()

    data class Metering(val on: Boolean) : Cmd()

    data object Close : Cmd()
}

/** The order used to read a device: the program first, because the device may stop answering after a couple of seconds (connection.md §1). */
fun readAllSequence(): List<Request> = buildList {
    add(Request.Connect)
    add(Request.Identify)
    addAll(programReadSequence())
    add(Request.GetCurrentPreset)
    for (n in 1..20) add(Request.GetPresetName(n))
    add(Request.GetLockState)
    add(Request.GetPresetSlots)
}

/** Page reads, then `0x12` as the original editor does after a bulk read. */
fun programReadSequence(): List<Request> =
    (0 until Program.PAGE_COUNT).map { Request.ReadProgramPage(it) } + Request.EndOfTransfer

/**
 * Store the current program into user preset [n] (1 = U01) under [name], as
 * the original editor does: `0x26` name, `0x21 [n]`, then `0x12`
 * (protocol.md §4.2). The worker sends each step after the previous ACK.
 */
fun storePresetSequence(n: Int, name: String): List<Request> = listOf(
    Request.SetPresetName(Names.presetField(name)),
    Request.StorePreset(n),
    Request.EndOfTransfer,
)

/** A connection to one device. [listener] runs on the worker thread. */
class Link private constructor(
    transport: Transport,
    config: LinkConfig,
    listener: (Event) -> Unit,
    clock: () -> Long,
) {
    private val cmds = LinkedBlockingQueue<Cmd>()
    private val pendingCount = AtomicInteger(0)
    private val thread = Thread(
        { Worker(transport, config, cmds, listener, pendingCount, clock).run() },
        "dsp206-link",
    ).apply { isDaemon = true }

    /** Queue a request. Setters with the same target replace queued ones. */
    fun send(req: Request) {
        cmds.add(Cmd.Send(req))
    }

    /** Queue the full read: handshake, identity, program, presets, lock state. */
    fun readAll() {
        cmds.add(Cmd.ReadAll)
    }

    /** Queue a full program read (after a recall, for example). */
    fun readProgram() {
        cmds.add(Cmd.ReadProgram(fresh = true))
    }

    /** Ask again only for the pages a previous read did not get. */
    fun retryMissingPages() {
        cmds.add(Cmd.ReadProgram(fresh = false))
    }

    fun setMetering(on: Boolean) {
        cmds.add(Cmd.Metering(on))
    }

    /** Requests queued or in flight. */
    val pending: Int get() = pendingCount.get()

    val isRunning: Boolean get() = thread.isAlive

    /** Send `0x11` (best effort) and stop the worker. Does not block. */
    fun close() {
        cmds.add(Cmd.Close)
    }

    /** Wait for the worker to stop. */
    fun join(timeoutMs: Long) {
        thread.join(timeoutMs)
    }

    companion object {
        fun start(
            transport: Transport,
            config: LinkConfig = LinkConfig(),
            clock: () -> Long = { System.nanoTime() / 1_000_000 },
            listener: (Event) -> Unit,
        ): Link = Link(transport, config, listener, clock).also { it.thread.start() }
    }
}

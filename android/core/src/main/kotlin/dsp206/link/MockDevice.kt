package dsp206.link

import dsp206.proto.Bytes
import dsp206.proto.Channel
import dsp206.proto.Codes
import dsp206.proto.Decoded
import dsp206.proto.Frame
import dsp206.proto.FrameDecoder
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.Units
import dsp206.proto.apply
import dsp206.proto.factoryDefault
import dsp206.proto.isMuted
import java.util.ArrayDeque
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import kotlin.math.pow
import kotlin.math.sin

/*
 * A simulated DSP 206 speaking the real protocol, in process. Used by the
 * tests (never the real device, see device-safety.md) and by the demo mode.
 * Port of crates/dsp206-link/src/mock.rs: replies follow what the real unit
 * sent in our captures; setters are applied to an in-memory Program and
 * acknowledged with 0x01.
 */

data class MockOptions(
    /** Stop answering after this many replies, like the real unit over the network (connection.md §1). */
    val replyLimit: Int? = null,
    /** Do not acknowledge setters. */
    val silentWrites: Boolean = false,
    /** Answer the first this-many frames with `0x02` (NAK) instead of handling them. */
    val nakFirst: Int = 0,
    /** How long a preset store or recall takes before its ACK. */
    val presetDelayMs: Long = 0,
    /** Reply latency of every frame (the unit answers USB requests in about 10 ms). */
    val latencyMs: Long = 0,
)

/** Everything the simulated device remembers. Lock the device to read or change it. */
class MockState {
    var program: Program = factoryDefault()

    /** F00 then U01..U20, indexed by preset number (`0x20`, `0x21`). */
    val presets: MutableList<Program> = MutableList(21) { factoryDefault() }
    var currentPreset = 0
    var tone = 0 to 0

    /** Every request received, in order. */
    val received = mutableListOf<Request>()
    var repliesSent = 0
    var naksSent = 0
}

class MockDevice(private val options: MockOptions = MockOptions(), private val deviceId: Int = 1) {
    private val state = MockState()
    private val started = System.nanoTime()

    /** Inspect or change the simulated state. */
    fun <T> state(block: MockState.() -> T): T = synchronized(state) { state.block() }

    /** Handle one frame from the PC; returns the reply frame, if any. */
    fun handle(frame: Frame): Frame? = synchronized(state) {
        val req = Request.parse(frame.cmd, frame.data) ?: return null
        state.received += req
        if (state.naksSent < options.nakFirst) {
            state.naksSent++
            return reply(Codes.NAK, Bytes.EMPTY)
        }
        val limited = options.replyLimit?.let { state.repliesSent >= it } ?: false
        val r = respond(req)
        if (limited) return null
        if (r != null) state.repliesSent++
        r
    }

    /** Extra time the unit needs before answering [frame]. */
    fun delayFor(frame: Frame): Long = when (frame.cmd) {
        0x20, 0x21 -> options.presetDelayMs
        else -> 0
    } + options.latencyMs

    private fun reply(cmd: Int, data: Bytes) = Frame(deviceId, 0, cmd, data)

    private fun respond(req: Request): Frame? {
        val st = state
        val ack = { if (options.silentWrites) null else reply(Codes.ACK, Bytes.EMPTY) }
        return when (req) {
            Request.Connect -> reply(0x10, Bytes.of(0x19))
            Request.Disconnect -> null
            Request.Identify -> reply(0x13, Bytes.ascii("DSP206 V0105P"))
            Request.GetCurrentPreset -> reply(0x14, Bytes.of(st.currentPreset))
            Request.GetLockState -> reply(0x2C, Bytes.of(0x00, 0x27, 0x0f, 0x00, 0x00, 0x00))
            Request.GetPresetSlots -> reply(0x22, Bytes.zeros(20))
            // `0x29 [i]` names U(i+1); i = 20 names the current program, as
            // the real unit does (protocol.md §4.2).
            is Request.GetPresetName -> {
                val name = when (req.n) {
                    in 1..20 -> st.presets[req.n].name
                    21 -> st.program.name
                    else -> return null
                }
                reply(0x29, Bytes.of(Codes.nameIndex(req.n)) + name)
            }
            is Request.ReadProgramPage -> {
                if (req.page !in 0 until Program.PAGE_COUNT) return null
                reply(0x24, Bytes.of(req.page) + st.program.toPages()[req.page])
            }
            Request.GetMeters -> reply(0x40, meters(st.program, (System.nanoTime() - started) / 1e9))
            Request.EndOfTransfer -> ack()
            // The original editor names its current program with the text it
            // sends in `0x26` (0x416444), then stores that program.
            is Request.SetPresetName -> {
                st.program.name = req.name
                ack()
            }
            is Request.StorePreset -> {
                if (req.n !in st.presets.indices) return null
                st.presets[req.n] = st.program.copy()
                ack()
            }
            is Request.RecallPreset -> {
                if (req.n !in st.presets.indices) return null
                st.program = st.presets[req.n].copy()
                st.currentPreset = req.n
                ack()
            }
            is Request.TestTone -> {
                st.tone = req.source to req.freq
                ack()
            }
            else -> {
                st.program.apply(req)
                ack()
            }
        }
    }

    /** Synthetic level meters: each unmuted channel moves slowly around a level set by its gain. */
    private fun meters(p: Program, t: Double): Bytes {
        val groups: List<Channel?> = listOf(
            Channel.IN_A, Channel.IN_B, null, null,
            Channel.OUT1, Channel.OUT2, Channel.OUT3, Channel.OUT4, Channel.OUT5, Channel.OUT6,
        )
        val d = Bytes.Builder()
        groups.forEachIndexed { i, ch ->
            val amp = if (ch != null && !p.isMuted(ch)) {
                val wobble = sin(t * 1.3 + i) * 6.0
                val dbu = -14.0 + Units.gainDb(p.gain(ch)) + wobble
                10.0.pow((dbu - 18.0) / 20.0).toFloat()
            } else {
                5e-6f
            }
            val bits = amp.toRawBits()
            // Little-endian bytes b0..b3 travel as [b2, b3, b1] (protocol.md §4.6).
            d.u8(bits shr 16).u8(bits shr 24).u8(bits shr 8)
        }
        repeat(8) { d.u8(0) }
        return d.build()
    }
}

/**
 * An in-process [Transport] to a [MockDevice]: frames are answered after the
 * device's delay, one at a time like the real unit.
 */
class MockTransport(val device: MockDevice) : Transport {
    private val decoder = FrameDecoder()
    private val queue = ArrayDeque<Pair<Long, ByteArray>>()
    private val lock = ReentrantLock()
    private val arrived = lock.newCondition()
    private var busyUntil = 0L

    @Volatile
    private var closed = false

    override fun send(frame: ByteArray) {
        if (closed) throw java.io.IOException("closed")
        decoder.push(frame)
        while (true) {
            val item = decoder.next() ?: break
            val f = (item as? Decoded.Ok)?.frame ?: continue
            val start = maxOf(nowMs(), busyUntil)
            val due = start + device.delayFor(f)
            busyUntil = due
            val reply = device.handle(f) ?: continue
            lock.withLock {
                queue.addLast(due to reply.encode())
                arrived.signalAll()
            }
        }
    }

    override fun recv(buf: ByteArray): Int {
        if (closed) return -1
        lock.withLock {
            val head = queue.peekFirst()
            val wait = if (head == null) POLL_MS else minOf(POLL_MS, head.first - nowMs())
            if (wait > 0) arrived.await(wait, TimeUnit.MILLISECONDS)
            val ready = queue.peekFirst() ?: return 0
            if (ready.first > nowMs()) return 0
            queue.removeFirst()
            ready.second.copyInto(buf)
            return ready.second.size
        }
    }

    override fun close() {
        closed = true
    }

    private fun nowMs() = System.nanoTime() / 1_000_000

    private companion object {
        const val POLL_MS = 5L
    }
}

package dsp206.link

import dsp206.proto.Bytes
import dsp206.proto.Codes
import dsp206.proto.Decoded
import dsp206.proto.FrameDecoder
import dsp206.proto.ProgramAssembler
import dsp206.proto.ProgramException
import dsp206.proto.Reply
import dsp206.proto.Request
import java.io.IOException
import java.util.ArrayDeque
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.atomic.AtomicInteger

/**
 * The link worker thread: one request in flight, timeouts, retries,
 * coalescing (protocol.md §3.2) and reply decoding. Port of
 * crates/dsp206-link/src/worker.rs.
 *
 * Requests that change the device (setters, presets) and the handshake go to
 * a priority queue, so a tap is never stuck behind a long program read.
 */
internal class Worker(
    private val io: Transport,
    private val cfg: LinkConfig,
    private val cmds: LinkedBlockingQueue<Cmd>,
    private val listener: (Event) -> Unit,
    private val pending: AtomicInteger,
    private val clock: () -> Long,
) {
    private class Inflight(val req: Request, val bytes: ByteArray, var deadline: Long, var tries: Int)

    /** Thrown to end the session with a reason. */
    private class Stop(val reason: CloseReason, val detail: String?) : Exception()

    private val writes = ArrayDeque<Request>()
    private val reads = ArrayDeque<Request>()
    private var inflight: Inflight? = null
    private var readyAt = clock()
    private val decoder = FrameDecoder()
    private var assembler = ProgramAssembler()
    private var readingProgram = false
    private var metering = false
    private var lastMeter = clock()
    private var misses = 0
    private var health = Health.ANSWERING
    private val buf = ByteArray(1024)

    fun run() {
        val closed = try {
            loop()
            Event.Closed(CloseReason.CLOSED)
        } catch (s: Stop) {
            Event.Closed(s.reason, s.detail)
        } catch (e: RuntimeException) {
            Event.Closed(CloseReason.RECEIVE_FAILED, e.toString())
        }
        io.close()
        pending.set(0)
        emit(closed)
    }

    private fun loop() {
        while (true) {
            while (true) {
                when (val cmd = cmds.poll() ?: break) {
                    is Cmd.Send -> enqueue(cmd.req)
                    Cmd.ReadAll -> {
                        startProgramRead(true)
                        readAllSequence().forEach { enqueue(it) }
                    }
                    is Cmd.ReadProgram -> {
                        startProgramRead(cmd.fresh)
                        programReadSequence().forEach { enqueue(it) }
                    }
                    is Cmd.Metering -> metering = cmd.on
                    Cmd.Close -> {
                        try {
                            write(frameBytes(Request.Disconnect))
                        } catch (_: Stop) {
                        }
                        return
                    }
                }
            }
            pump()
            tick()
            pending.set(writes.size + reads.size + (if (inflight != null) 1 else 0))
        }
    }

    private fun emit(ev: Event) {
        try {
            listener(ev)
        } catch (_: RuntimeException) {
            // A failing listener must not kill the link.
        }
    }

    /** [fresh] restarts from scratch; otherwise pages already received are kept and only the missing ones are requested again. */
    private fun startProgramRead(fresh: Boolean) {
        if (fresh || assembler.isComplete) assembler = ProgramAssembler()
        readingProgram = true
    }

    private fun enqueue(req: Request) {
        if (req is Request.ReadProgramPage && req.page !in assembler.missingPages()) return
        val queue = if (isPriority(req)) writes else reads
        val key = req.coalesceKey
        if (key != null) {
            val list = queue.toMutableList()
            val i = list.indexOfFirst { it.coalesceKey == key }
            if (i >= 0) {
                list[i] = req
                queue.clear()
                queue.addAll(list)
                return
            }
        } else if (req in queue) {
            return
        }
        queue.addLast(req)
    }

    private fun write(bytes: ByteArray) {
        emit(Event.Trace(outgoing = true, bytes = Bytes.copyOf(bytes)))
        try {
            io.send(bytes)
        } catch (e: IOException) {
            throw Stop(CloseReason.SEND_FAILED, e.message)
        }
    }

    /** Read whatever the connection has and handle complete frames. */
    private fun pump() {
        val n = try {
            io.recv(buf)
        } catch (e: IOException) {
            throw Stop(CloseReason.RECEIVE_FAILED, e.message)
        }
        if (n < 0) throw Stop(CloseReason.REMOTE_CLOSED, null)
        if (n > 0) decoder.push(buf, 0, n)
        while (true) {
            val item = decoder.next() ?: break
            val frame = (item as? Decoded.Ok)?.frame ?: continue
            emit(Event.Trace(outgoing = false, bytes = Bytes.copyOf(frame.encode())))
            if (frame.src != cfg.deviceId) continue
            answered()
            if (frame.cmd == Codes.NAK) {
                // The device got a damaged frame: send the request again.
                inflight?.deadline = clock()
                continue
            }
            val reply = Reply.parse(frame)
            val inf = inflight
            val done = inf != null && when (val code = inf.req.replyCode) {
                null -> true
                else -> code == frame.cmd && matchesIndex(inf.req, reply)
            }
            // Store the reply (e.g. the last page) before finishing the request.
            handleReply(reply)
            if (done) {
                inflight = null
                readyAt = clock() + cfg.gapMs
                afterRequest(inf.req)
            }
        }
    }

    private fun answered() {
        misses = 0
        if (health != Health.ANSWERING) {
            health = Health.ANSWERING
            emit(Event.HealthChanged(Health.ANSWERING))
        }
    }

    private fun missed() {
        misses++
        if (misses >= cfg.silentAfter && health != Health.SILENT) {
            health = Health.SILENT
            emit(Event.HealthChanged(Health.SILENT))
        }
    }

    private fun handleReply(reply: Reply) {
        val ev = when (reply) {
            is Reply.Connected -> Event.Handshake(reply.status)
            is Reply.Identity -> Event.Identity(reply.text)
            is Reply.CurrentPreset -> Event.CurrentPreset(reply.n)
            is Reply.PresetSlots -> Event.PresetSlots(reply.slots)
            is Reply.PresetName -> Event.PresetName(reply.index, reply.name)
            is Reply.LockState -> Event.LockState(reply.state)
            is Reply.MeterLevels -> Event.MeterLevels(reply.meters)
            is Reply.ProgramPage -> {
                try {
                    assembler.insert(reply.page, reply.data)
                } catch (_: ProgramException) {
                    return
                }
                Event.Page(reply.page, reply.data)
            }
            Reply.Ack, Reply.Nak, is Reply.Other -> return
        }
        emit(ev)
    }

    /** Called when a request completes or is abandoned. */
    private fun afterRequest(req: Request?) {
        if (req !is Request.ReadProgramPage || !readingProgram) return
        if (reads.any { it is Request.ReadProgramPage }) return
        readingProgram = false
        val program = assembler.program()
        emit(if (program != null) Event.ProgramRead(program) else Event.ProgramIncomplete(assembler.missingPages()))
    }

    private fun timeoutFor(req: Request): Long = when {
        req is Request.StorePreset -> cfg.storeTimeoutMs
        req is Request.RecallPreset -> cfg.recallTimeoutMs
        req.isWrite -> cfg.writeTimeoutMs
        else -> cfg.readTimeoutMs
    }

    private fun tick() {
        val now = clock()
        val inf = inflight
        if (inf != null) {
            if (now < inf.deadline) return
            val expectsReply = inf.req.replyCode != null
            if (expectsReply && inf.tries < cfg.retries && isRetried(inf.req)) {
                inf.tries++
                inf.deadline = now + timeoutFor(inf.req)
                write(inf.bytes)
                return
            }
            inflight = null
            readyAt = now + cfg.gapMs
            if (expectsReply) missed()
            when {
                inf.req == Request.GetMeters -> {}
                inf.req.isWrite -> emit(Event.Unconfirmed(inf.req))
                expectsReply -> emit(Event.Timeout(inf.req))
            }
            afterRequest(inf.req)
        }
        if (now < readyAt) return
        val next = writes.pollFirst() ?: reads.pollFirst() ?: if (metering && now - lastMeter >= cfg.meterIntervalMs) {
            lastMeter = now
            Request.GetMeters
        } else {
            return
        }
        val bytes = frameBytes(next)
        write(bytes)
        inflight = Inflight(next, bytes, now + timeoutFor(next), 0)
    }

    private fun frameBytes(req: Request): ByteArray = req.frame(cfg.deviceId).encode()

    private companion object {
        /** Requests served before any queued read. */
        fun isPriority(req: Request) = req.isWrite || req == Request.Connect

        /**
         * Meter polls are simply skipped. A preset store or recall already had a
         * long wait, and sending it again while the unit is busy with its flash
         * helps nothing.
         */
        fun isRetried(req: Request) =
            req != Request.GetMeters && req !is Request.StorePreset && req !is Request.RecallPreset

        /** Replies that carry an index must match the request's index. */
        fun matchesIndex(req: Request, reply: Reply): Boolean = when {
            req is Request.ReadProgramPage && reply is Reply.ProgramPage -> req.page == reply.page
            req is Request.GetPresetName && reply is Reply.PresetName -> req.n == reply.index
            else -> true
        }
    }
}

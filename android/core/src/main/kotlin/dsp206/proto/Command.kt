package dsp206.proto

/**
 * Typed requests and replies (protocol.md §4).
 *
 * Setter payloads follow protocol.md §4.3 exactly. Multi-byte values are
 * little-endian u16. Channel numbers are global: 0 = InA, 1 = InB,
 * 2..7 = Out1..Out6.
 */
object Codes {
    /** Reply to setters and preset operations (protocol.md §3.3). */
    const val ACK = 0x01

    /** Reply to a frame the device could not take, probably a damaged one (protocol.md §3.3). */
    const val NAK = 0x02

    /** Command codes that only read device state (claude/rules/device-safety.md). */
    val READ_ONLY: List<Int> = listOf(0x10, 0x11, 0x13, 0x14, 0x22, 0x27, 0x29, 0x2C, 0x40)

    /** Every command code a [Request] can produce, sorted. */
    val ALL: List<Int> = listOf(
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x20, 0x21, 0x22, 0x26, 0x27, 0x29, 0x2A, 0x2C, 0x30, 0x31,
        0x32, 0x33, 0x34, 0x35, 0x36, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x48,
        0x49,
    )

    /**
     * Wire index of user preset [n] in `0x29` and in the `0x22` table: they
     * count from 0, so U01 is 0 and U20 is 19, while `0x14`, `0x20` and `0x21`
     * use 1 for U01 (protocol.md §4.2). Index 20 of `0x29` answers the current
     * program's name, which reads back as preset 21.
     */
    fun nameIndex(n: Int): Int = (n - 1) and 0xff
}

/** Test-tone sources (`TfmTest` radio button tags) [code]. */
enum class ToneSource(val raw: Int) {
    ANALOG_INPUT(0),
    PINK_NOISE(1),
    WHITE_NOISE(2),
    SINE(3),
}

/** Queued setters with the same key replace each other (protocol.md §3.2). */
data class CoalesceKey(val code: Int, val ch: Int, val sub: Int)

/** Requests sent by the PC. */
sealed class Request {
    // Session and information (protocol.md §4.1)
    data object Connect : Request()

    data object Disconnect : Request()

    data object Identify : Request()

    data object GetCurrentPreset : Request()

    data object GetLockState : Request()

    data object GetMeters : Request()

    /** `0x12`, sent after a bulk read. Effect not confirmed. */
    data object EndOfTransfer : Request()

    // Presets and bulk transfer (§4.2)
    data object GetPresetSlots : Request()

    /** `0x29`: name of user preset [n] (1 = U01 … 20 = U20). The wire index counts from 0. */
    data class GetPresetName(val n: Int) : Request()

    data class ReadProgramPage(val page: Int) : Request()

    /** `0x26`: name of the current program (14 bytes), sent before a store. */
    data class SetPresetName(val name: Bytes) : Request()

    /** `0x21 [n]`: store the current program into user preset [n] (1 = U01). */
    data class StorePreset(val n: Int) : Request()

    /** `0x20 [n]`: load preset [n] (0 = F00, 1 = U01) as the current program. */
    data class RecallPreset(val n: Int) : Request()

    // Parameter setters (§4.3)
    data class SetGain(val ch: Channel, val raw: Int) : Request()

    data class SetMute(val ch: Channel, val on: Boolean) : Request()

    data class SetPolarity(val ch: Channel, val inverted: Boolean) : Request()

    data class SetDelay(val ch: Channel, val raw: Int) : Request()

    data class SetDelayUnit(val unit: Int) : Request()

    data class SetPeqBand(val ch: Channel, val band: Int, val value: PeqBand, val bypass: Boolean) : Request()

    data class SetPeqBypass(val ch: Channel, val on: Boolean) : Request()

    data class SetGeqBand(val ch: Channel, val band: Int, val gain: Int) : Request()

    data class SetGeqBypass(val ch: Channel, val on: Boolean) : Request()

    data class SetHighPass(val ch: Channel, val filter: CutFilter) : Request()

    data class SetLowPass(val ch: Channel, val filter: CutFilter) : Request()

    data class SetGate(val ch: Channel, val gate: Gate) : Request()

    data class SetCompressor(val ch: Channel, val comp: Compressor) : Request()

    data class SetLimiter(val ch: Channel, val limiter: Limiter) : Request()

    data class SetRouting(val ch: Channel, val mask: Int) : Request()

    data class SetMixGain(val ch: Channel, val input: Int, val raw: Int) : Request()

    data class SetLink(val ch: Channel, val mask: Int) : Request()

    data class CopyChannel(val src: Channel, val dst: Channel) : Request()

    /** 8-byte channel name field. */
    data class SetChannelName(val ch: Channel, val name: Bytes) : Request()

    /** Test-tone generator: [source] is a [ToneSource] value, [freq] an index into [Units.GEQ_BANDS_HZ]. */
    data class TestTone(val source: Int, val freq: Int) : Request()

    /** Command byte. */
    val code: Int
        get() = when (this) {
            Connect -> 0x10
            Disconnect -> 0x11
            EndOfTransfer -> 0x12
            Identify -> 0x13
            GetCurrentPreset -> 0x14
            is SetDelayUnit -> 0x15
            is RecallPreset -> 0x20
            is StorePreset -> 0x21
            GetPresetSlots -> 0x22
            is SetPresetName -> 0x26
            is ReadProgramPage -> 0x27
            is GetPresetName -> 0x29
            is CopyChannel -> 0x2A
            GetLockState -> 0x2C
            is SetCompressor -> 0x30
            is SetLowPass -> 0x31
            is SetHighPass -> 0x32
            is SetPeqBand -> 0x33
            is SetGain -> 0x34
            is SetMute -> 0x35
            is SetPolarity -> 0x36
            is SetDelay -> 0x38
            is TestTone -> 0x39
            is SetRouting -> 0x3A
            is SetLink -> 0x3B
            is SetPeqBypass -> 0x3C
            is SetChannelName -> 0x3D
            is SetGate -> 0x3E
            is SetLimiter -> 0x3F
            GetMeters -> 0x40
            is SetMixGain -> 0x41
            is SetGeqBand -> 0x48
            is SetGeqBypass -> 0x49
        }

    /** Payload bytes (everything between `cmd` and `10 03`). */
    fun payload(): Bytes {
        val d = Bytes.Builder()
        fun flag(on: Boolean) = if (on) 1 else 0
        when (this) {
            Connect, Disconnect, EndOfTransfer, Identify, GetCurrentPreset, GetLockState, GetMeters,
            GetPresetSlots,
            -> {}
            is GetPresetName -> d.u8(Codes.nameIndex(n))
            is ReadProgramPage -> d.u8(page)
            is StorePreset -> d.u8(n)
            is RecallPreset -> d.u8(n)
            is SetDelayUnit -> d.u8(unit)
            is SetPresetName -> d.bytes(name)
            is SetGain -> d.u8(ch.index).u16(raw)
            is SetDelay -> d.u8(ch.index).u16(raw)
            is SetMute -> d.u8(ch.index).u8(flag(on))
            is SetPeqBypass -> d.u8(ch.index).u8(flag(on))
            is SetGeqBypass -> d.u8(ch.index).u8(flag(on))
            is SetPolarity -> d.u8(ch.index).u8(flag(inverted))
            is SetPeqBand -> d.u8(ch.index).u8(band).words(value.gain, value.freq)
                .u8(value.q).u8(value.kind).u8(flag(bypass))
            is SetGeqBand -> d.u8(ch.index).u8(band).u16(gain)
            is SetHighPass -> d.u8(ch.index).u16(filter.freq).u8(filter.slope)
            is SetLowPass -> d.u8(ch.index).u16(filter.freq).u8(filter.slope)
            is SetGate -> d.u8(ch.index).words(gate.attack, gate.hold, gate.release, gate.threshold)
            is SetCompressor -> d.u8(ch.index)
                .words(comp.ratio, comp.attack, comp.release, comp.knee, comp.threshold)
            is SetLimiter -> d.u8(ch.index)
                .words(limiter.attack, limiter.release, limiter.knee, limiter.threshold)
            is SetRouting -> d.u8(ch.index).u8(mask)
            is SetLink -> d.u8(ch.index).u8(mask)
            is SetMixGain -> d.u8(ch.index).u8(input).u16(raw)
            is CopyChannel -> d.u8(src.index).u8(dst.index)
            is SetChannelName -> d.u8(ch.index).bytes(name)
            is TestTone -> d.u8(source).u8(freq)
        }
        return d.build()
    }

    /** The frame for device [dev]. */
    fun frame(dev: Int): Frame = Frame.request(dev, code, payload())

    /** True when the command cannot change sound or device state. */
    val isReadOnly: Boolean get() = code in Codes.READ_ONLY

    /**
     * Command byte of the reply that completes this request, when known.
     * Setters and preset operations are acknowledged with [Codes.ACK]
     * (seen on the unit for `0x35`, protocol.md §3.3).
     */
    val replyCode: Int?
        get() = when (this) {
            is ReadProgramPage -> 0x24
            Connect, Identify, GetCurrentPreset, GetLockState, GetMeters, GetPresetSlots, is GetPresetName -> code
            Disconnect, EndOfTransfer -> null
            else -> Codes.ACK
        }

    /** True for requests that change the device (answered by [Codes.ACK]). */
    val isWrite: Boolean get() = replyCode == Codes.ACK

    /** Queued setters with the same key replace each other (protocol.md §3.2). Reads are never coalesced. */
    val coalesceKey: CoalesceKey?
        get() = when (this) {
            is SetPeqBand -> CoalesceKey(code, ch.index, band)
            is SetGeqBand -> CoalesceKey(code, ch.index, band)
            is SetMixGain -> CoalesceKey(code, ch.index, input)
            is SetGain -> CoalesceKey(code, ch.index, 0)
            is SetMute -> CoalesceKey(code, ch.index, 0)
            is SetPolarity -> CoalesceKey(code, ch.index, 0)
            is SetDelay -> CoalesceKey(code, ch.index, 0)
            is SetPeqBypass -> CoalesceKey(code, ch.index, 0)
            is SetGeqBypass -> CoalesceKey(code, ch.index, 0)
            is SetHighPass -> CoalesceKey(code, ch.index, 0)
            is SetLowPass -> CoalesceKey(code, ch.index, 0)
            is SetGate -> CoalesceKey(code, ch.index, 0)
            is SetCompressor -> CoalesceKey(code, ch.index, 0)
            is SetLimiter -> CoalesceKey(code, ch.index, 0)
            is SetRouting -> CoalesceKey(code, ch.index, 0)
            is SetLink -> CoalesceKey(code, ch.index, 0)
            is SetChannelName -> CoalesceKey(code, ch.index, 0)
            is SetDelayUnit, is TestTone -> CoalesceKey(code, 0, 0)
            else -> null
        }

    companion object {
        /** Decode a request from its command byte and payload (inverse of [payload]). Used by the simulated device. */
        fun parse(cmd: Int, d: Bytes): Request? {
            fun b(i: Int): Int? = d.getOrNull(i)
            fun w(i: Int): Int? = if (i + 1 < d.size) d.u16(i) else null
            fun ch(i: Int): Channel? = b(i)?.let { Channel.of(it) }
            fun <T> exact0(r: T): T? = if (d.isEmpty()) r else null
            return when (cmd) {
                0x10 -> exact0(Connect)
                0x11 -> exact0(Disconnect)
                0x12 -> exact0(EndOfTransfer)
                0x13 -> exact0(Identify)
                0x14 -> exact0(GetCurrentPreset)
                0x15 -> SetDelayUnit(b(0) ?: return null)
                0x20 -> RecallPreset(b(0) ?: return null)
                0x21 -> StorePreset(b(0) ?: return null)
                0x22 -> exact0(GetPresetSlots)
                0x26 -> if (d.size == Program.PROGRAM_NAME_LEN) SetPresetName(d) else null
                0x27 -> ReadProgramPage(b(0) ?: return null)
                0x29 -> GetPresetName(((b(0) ?: return null) + 1) and 0xff)
                0x2A -> CopyChannel(ch(0) ?: return null, ch(1) ?: return null)
                0x2C -> exact0(GetLockState)
                0x30 -> SetCompressor(
                    ch(0) ?: return null,
                    Compressor(
                        w(1) ?: return null, w(3) ?: return null, w(5) ?: return null,
                        w(7) ?: return null, w(9) ?: return null,
                    ),
                )
                0x31, 0x32 -> {
                    val c = ch(0) ?: return null
                    val f = CutFilter(w(1) ?: return null, b(3) ?: return null)
                    if (cmd == 0x31) SetLowPass(c, f) else SetHighPass(c, f)
                }
                0x33 -> SetPeqBand(
                    ch(0) ?: return null,
                    b(1) ?: return null,
                    PeqBand(w(2) ?: return null, w(4) ?: return null, b(6) ?: return null, b(7) ?: return null),
                    (b(8) ?: return null) != 0,
                )
                0x34 -> SetGain(ch(0) ?: return null, w(1) ?: return null)
                0x35 -> SetMute(ch(0) ?: return null, (b(1) ?: return null) != 0)
                0x36 -> SetPolarity(ch(0) ?: return null, (b(1) ?: return null) != 0)
                0x38 -> SetDelay(ch(0) ?: return null, w(1) ?: return null)
                0x39 -> TestTone(b(0) ?: return null, b(1) ?: return null)
                0x3A -> SetRouting(ch(0) ?: return null, b(1) ?: return null)
                0x3B -> SetLink(ch(0) ?: return null, b(1) ?: return null)
                0x3C -> SetPeqBypass(ch(0) ?: return null, (b(1) ?: return null) != 0)
                0x3D -> {
                    val c = ch(0) ?: return null
                    if (d.size < 1 + Program.NAME_LEN) return null
                    SetChannelName(c, d.slice(1, 1 + Program.NAME_LEN))
                }
                0x3E -> SetGate(
                    ch(0) ?: return null,
                    Gate(w(1) ?: return null, w(3) ?: return null, w(5) ?: return null, w(7) ?: return null),
                )
                0x3F -> SetLimiter(
                    ch(0) ?: return null,
                    Limiter(w(1) ?: return null, w(3) ?: return null, w(5) ?: return null, w(7) ?: return null),
                )
                0x40 -> exact0(GetMeters)
                0x41 -> SetMixGain(ch(0) ?: return null, b(1) ?: return null, w(2) ?: return null)
                0x48 -> SetGeqBand(ch(0) ?: return null, b(1) ?: return null, w(2) ?: return null)
                0x49 -> SetGeqBypass(ch(0) ?: return null, (b(1) ?: return null) != 0)
                else -> null
            }
        }
    }
}

/** Decoded `0x40` meter reply (protocol.md §4.6). */
data class Meters(
    /** InA, InB in dBu. */
    val inputs: List<Double>,
    /** Out1..Out6 in dBu. */
    val outputs: List<Double>,
    /** Bytes 30..32, compressor/limiter activity (meaning not confirmed). */
    val status: Int,
) {
    /** Level of [ch] in dBu. */
    fun of(ch: Channel): Double = if (ch.isInput) inputs[ch.index] else outputs[ch.index - 2]

    companion object {
        const val MIN_LEN = 32

        fun parse(data: Bytes): Meters? {
            if (data.size < MIN_LEN) return null
            fun group(i: Int) = Units.meterDbu(data[3 * i], data[3 * i + 1], data[3 * i + 2])
            return Meters(
                inputs = listOf(group(0), group(1)),
                // Groups 2 and 3 are unused on the DSP 206.
                outputs = (4..9).map { group(it) },
                status = data.u16(30),
            )
        }
    }
}

/** Replies sent by the device. */
sealed class Reply {
    data object Ack : Reply()

    /** `0x02`, seen once when the device was busy (meaning not confirmed). */
    data object Nak : Reply()

    data class Connected(val status: Int) : Reply()

    data class Identity(val text: String) : Reply()

    data class CurrentPreset(val n: Int) : Reply()

    data class PresetSlots(val slots: Bytes) : Reply()

    /** [index] is the preset number (1 = U01); 21 is the current program. */
    data class PresetName(val index: Int, val name: String) : Reply()

    data class ProgramPage(val page: Int, val data: Bytes) : Reply()

    data class LockState(val state: Bytes) : Reply()

    data class MeterLevels(val meters: Meters) : Reply()

    /** Anything not decoded yet, kept whole. */
    data class Other(val frame: Frame) : Reply()

    companion object {
        fun parse(frame: Frame): Reply {
            val d = frame.data
            return when {
                frame.cmd == 0x01 -> Ack
                frame.cmd == 0x02 -> Nak
                frame.cmd == 0x10 && d.size >= 1 -> Connected(d[0])
                frame.cmd == 0x13 -> Identity(String(d.toByteArray(), Charsets.UTF_8).trimEnd())
                frame.cmd == 0x14 && d.size >= 1 -> CurrentPreset(d[0])
                frame.cmd == 0x22 -> PresetSlots(d)
                frame.cmd == 0x29 && d.size >= 1 -> PresetName((d[0] + 1) and 0xff, Names.text(d.slice(1)))
                frame.cmd == 0x24 && d.size >= 1 -> ProgramPage(d[0], d.slice(1))
                frame.cmd == 0x2C -> LockState(d)
                frame.cmd == 0x40 -> Meters.parse(d)?.let { MeterLevels(it) } ?: Other(frame)
                else -> Other(frame)
            }
        }
    }
}

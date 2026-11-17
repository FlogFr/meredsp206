package dsp206.proto

/*
 * Editing a Program: factory defaults, applying setter requests locally,
 * generating the requests that reproduce a program on a device, and the
 * native program file format. Port of dsp206-proto/src/edit.rs.
 */

private fun bit(word: Int, bit: Int): Boolean = word and (1 shl bit) != 0

private fun withBit(word: Int, bit: Int, on: Boolean): Int =
    if (on) word or (1 shl bit) else word and (1 shl bit).inv()

/** Native program file: magic + the 928-byte block. */
val FILE_MAGIC: Bytes = Bytes.of(0x44, 0x53, 0x50, 0x32, 0x30, 0x36, 0x50, 0x01) // "DSP206P\x01"

private fun peqDefault(freq: Int) = PeqBand(gain = 120, freq = freq, q = 35, kind = 0)

/**
 * The factory "Default Preset" (memory-map.md §5). Pages that were not
 * captured are assumed to follow the same pattern; output routing of the
 * even outputs (InB) is an assumption.
 */
fun factoryDefault(): Program {
    val p = Program.blank()
    p.header = Bytes.of(0xff, 0x00)
    p.name = Names.field("Default Preset", Program.PROGRAM_NAME_LEN)
    val inFreqs = listOf(41, 71, 101, 140, 170, 200, 240, 270)
    for ((k, ch) in Channel.INPUTS.withIndex()) {
        p.setChannelNameField(ch, Names.field(ch.label, Program.NAME_LEN))
        p.setGate(ch, Gate(attack = 0, hold = 99, release = 99, threshold = 0))
        for (band in 0 until Program.GEQ_BANDS) p.setGeq(ch, band, 120)
        inFreqs.forEachIndexed { band, f -> p.setPeq(ch, band, peqDefault(f)) }
        p.setHighPass(ch, CutFilter(0, 0))
        p.setLowPass(ch, CutFilter(300, 0))
        p.setGain(ch, 280)
        p.setLink(ch, 1 shl k)
    }
    val outFreqs = listOf(31, 63, 95, 127, 158, 190, 222, 253, 285)
    for ((k, ch) in Channel.OUTPUTS.withIndex()) {
        p.setChannelNameField(ch, Names.field("Out${k + 1}", Program.NAME_LEN))
        p.setRouting(ch, if (k % 2 == 0) 1 else 2)
        p.setMix(ch, 0, 280)
        p.setMix(ch, 1, 280)
        p.setHighPass(ch, CutFilter(0, 0))
        p.setLowPass(ch, CutFilter(300, 0))
        outFreqs.forEachIndexed { band, f -> p.setPeq(ch, band, peqDefault(f)) }
        p.setCompressor(ch, Compressor(ratio = 0, attack = 49, release = 499, knee = 0, threshold = 220))
        p.setLimiter(ch, Limiter(attack = 49, release = 499, knee = 0, threshold = 220))
        p.setGain(ch, 280)
        p.setLink(ch, 1 shl k)
    }
    return p
}

fun Program.isMuted(ch: Channel): Boolean = bit(if (ch.isInput) inputMute else outputMute, ch.domainBit)

fun Program.peqBypassed(ch: Channel): Boolean =
    bit(if (ch.isInput) inputPeqBypass else outputPeqBypass, ch.domainBit)

fun Program.peqBandBypassed(ch: Channel, band: Int): Boolean {
    val k = ch.domainBit
    return bit(if (ch.isInput) inputPeqBandBypass(k) else outputPeqBandBypass(k), band)
}

fun Program.geqBypassed(ch: Channel): Boolean = ch.isInput && bit(geqBypass, ch.domainBit)

/**
 * Channels that follow [ch]: every channel of the same domain whose bit is
 * set in [ch]'s link mask, always including [ch] itself.
 */
fun Program.linkedChannels(ch: Channel): List<Channel> {
    val mask = link(ch)
    val out = ch.domain.filterIndexed { i, _ -> bit(mask, i) }.toMutableList()
    if (ch !in out) out.add(0, ch)
    return out
}

private fun Program.setMuteBit(c: Channel, on: Boolean) {
    if (c.isInput) inputMute = withBit(inputMute, c.domainBit, on) else outputMute = withBit(outputMute, c.domainBit, on)
}

/**
 * Apply a setter to the local model. Processing setters also update the
 * linked channels, as the original editor does. Returns `true` when the model
 * changed. Reads, preset commands and invalid targets change nothing.
 */
fun Program.apply(req: Request): Boolean {
    val before = copy()
    applyInner(req)
    return this != before
}

private fun Program.applyInner(req: Request) {
    when (req) {
        is Request.SetGain -> linkedChannels(req.ch).forEach { setGain(it, req.raw and 0xffff) }
        is Request.SetPolarity -> linkedChannels(req.ch).forEach { setPolarity(it, if (req.inverted) 1 else 0) }
        is Request.SetDelay -> linkedChannels(req.ch).forEach { setDelay(it, req.raw and 0xffff) }
        is Request.SetMute -> linkedChannels(req.ch).forEach { setMuteBit(it, req.on) }
        is Request.SetDelayUnit -> delayUnit = req.unit and 0xff
        is Request.SetPeqBand -> {
            for (c in linkedChannels(req.ch)) {
                if (req.band !in 0 until peqBandCount(c)) return
                setPeq(c, req.band, req.value)
                val k = c.domainBit
                if (c.isInput) {
                    setInputPeqBandBypass(k, withBit(inputPeqBandBypass(k), req.band, req.bypass))
                } else {
                    setOutputPeqBandBypass(k, withBit(outputPeqBandBypass(k), req.band, req.bypass))
                }
            }
        }
        is Request.SetPeqBypass -> for (c in linkedChannels(req.ch)) {
            if (c.isInput) {
                inputPeqBypass = withBit(inputPeqBypass, c.domainBit, req.on)
            } else {
                outputPeqBypass = withBit(outputPeqBypass, c.domainBit, req.on)
            }
        }
        is Request.SetGeqBand -> if (req.ch.isInput && req.band in 0 until Program.GEQ_BANDS) {
            linkedChannels(req.ch).forEach { setGeq(it, req.band, req.gain) }
        }
        is Request.SetGeqBypass -> if (req.ch.isInput) {
            linkedChannels(req.ch).forEach { geqBypass = withBit(geqBypass, it.domainBit, req.on) }
        }
        is Request.SetHighPass -> linkedChannels(req.ch).forEach { setHighPass(it, req.filter) }
        is Request.SetLowPass -> linkedChannels(req.ch).forEach { setLowPass(it, req.filter) }
        is Request.SetGate -> if (req.ch.isInput) linkedChannels(req.ch).forEach { setGate(it, req.gate) }
        is Request.SetCompressor -> if (!req.ch.isInput) linkedChannels(req.ch).forEach { setCompressor(it, req.comp) }
        is Request.SetLimiter -> if (!req.ch.isInput) linkedChannels(req.ch).forEach { setLimiter(it, req.limiter) }
        is Request.SetRouting -> if (!req.ch.isInput) setRouting(req.ch, req.mask and 0xff)
        is Request.SetMixGain -> if (!req.ch.isInput && req.input in 0..1) setMix(req.ch, req.input, req.raw)
        is Request.SetLink -> setLink(req.ch, req.mask and 0xff)
        is Request.SetChannelName -> setChannelNameField(req.ch, req.name)
        is Request.CopyChannel -> if (req.src.isInput == req.dst.isInput && req.src != req.dst) {
            copyChannel(req.src, req.dst)
        }
        else -> {}
    }
}

/**
 * Copy every processing setting of [src] to [dst] (same domain), keeping the
 * destination's name, link mask and routing.
 */
private fun Program.copyChannel(src: Channel, dst: Channel) {
    val name = channelNameField(dst)
    val link = link(dst)
    if (src.isInput) {
        val bytes = toBytes()
        for (i in 0 until Program.INPUT_STRIDE) setU8(base(dst) + i, bytes[base(src) + i].toInt())
    } else {
        val routing = routing(dst)
        val mix = listOf(mix(dst, 0), mix(dst, 1))
        val bytes = toBytes()
        for (i in 0 until Program.OUTPUT_STRIDE) setU8(base(dst) + i, bytes[base(src) + i].toInt())
        setRouting(dst, routing)
        setMix(dst, 0, mix[0])
        setMix(dst, 1, mix[1])
    }
    setChannelNameField(dst, name)
    setLink(dst, link)
    // Flags stored in the global masks.
    val s = src.domainBit
    val d = dst.domainBit
    fun copied(mask: Int) = withBit(mask, d, bit(mask, s))
    if (src.isInput) {
        inputMute = copied(inputMute)
        inputPeqBypass = copied(inputPeqBypass)
        geqBypass = copied(geqBypass)
        setInputPeqBandBypass(d, inputPeqBandBypass(s))
    } else {
        outputMute = copied(outputMute)
        outputPeqBypass = copied(outputPeqBypass)
        setOutputPeqBandBypass(d, outputPeqBandBypass(s))
    }
}

/** Setter requests that reproduce this program on a device, link masks last so earlier setters do not propagate unexpectedly. */
fun Program.syncRequests(): List<Request> {
    val out = mutableListOf<Request>(Request.SetDelayUnit(delayUnit and 0xff))
    val links = mutableListOf<Request>()
    for (ch in Channel.entries) {
        out += Request.SetChannelName(ch, channelNameField(ch))
        out += Request.SetGain(ch, gain(ch))
        out += Request.SetMute(ch, isMuted(ch))
        out += Request.SetPolarity(ch, polarity(ch) != 0)
        out += Request.SetDelay(ch, delay(ch))
        out += Request.SetHighPass(ch, highPass(ch))
        out += Request.SetLowPass(ch, lowPass(ch))
        for (band in 0 until peqBandCount(ch)) {
            out += Request.SetPeqBand(ch, band, peq(ch, band), peqBandBypassed(ch, band))
        }
        out += Request.SetPeqBypass(ch, peqBypassed(ch))
        if (ch.isInput) {
            out += Request.SetGate(ch, gate(ch))
            for (band in 0 until Program.GEQ_BANDS) out += Request.SetGeqBand(ch, band, geq(ch, band))
            out += Request.SetGeqBypass(ch, geqBypassed(ch))
        } else {
            out += Request.SetRouting(ch, routing(ch) and 0xff)
            for (input in 0..1) out += Request.SetMixGain(ch, input, mix(ch, input))
            out += Request.SetCompressor(ch, compressor(ch))
            out += Request.SetLimiter(ch, limiter(ch))
        }
        links += Request.SetLink(ch, link(ch) and 0xff)
    }
    return out + links
}

/** The setters needed to turn a device holding this program into [target]. */
fun Program.diffRequests(target: Program): List<Request> {
    val current = syncRequests().toHashSet()
    return target.syncRequests().filter { it !in current }
}

/** Serialize to the native `.dsp206` file format. */
fun Program.toFile(): ByteArray = FILE_MAGIC.toByteArray() + toBytes()

fun programFromFile(bytes: ByteArray): Program {
    val magic = FILE_MAGIC.toByteArray()
    if (bytes.size != magic.size + Program.SIZE || !bytes.copyOf(magic.size).contentEquals(magic)) {
        throw ProgramException(ProgramError.BadFile)
    }
    return Program.fromBytes(bytes.copyOfRange(magic.size, bytes.size))
}

/**
 * The setters that join [other] to (or remove it from) [ch]'s link group, as
 * the GTK editor's link buttons do: every member of the new group gets the
 * group mask, and channels that leave it get their own bit back.
 */
fun Program.linkRequests(ch: Channel, other: Channel, join: Boolean): List<Request> {
    if (ch.isInput != other.isInput || ch == other) return emptyList()
    val old = linkedChannels(ch)
    val group = old.toMutableList()
    if (join && other !in group) group += other
    if (!join) group.remove(other)
    val mask = group.fold(0) { acc, c -> acc or (1 shl c.domainBit) }
    val out = group.map { Request.SetLink(it, mask) }.toMutableList()
    for (c in (old + other).distinct()) {
        if (c !in group) out += Request.SetLink(c, 1 shl c.domainBit)
    }
    return out
}

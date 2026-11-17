package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import dsp206.editor.Format
import dsp206.proto.Channel
import dsp206.proto.Compressor
import dsp206.proto.CutFilter
import dsp206.proto.Gate
import dsp206.proto.Limiter
import dsp206.proto.Names
import dsp206.proto.PeqType
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.Units
import dsp206.proto.geqBypassed
import dsp206.proto.isMuted
import dsp206.proto.linkedChannels
import dsp206.proto.peqBandBypassed
import dsp206.proto.peqBypassed
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.R
import kotlin.math.roundToInt

/** Detailed editor of one channel. Every change is sent at once. */
@Composable
fun ChannelScreen(vm: DspViewModel, modifier: Modifier) {
    DisposableEffect(vm) {
        vm.setMeterView(true)
        onDispose { vm.setMeterView(false) }
    }
    val ch = vm.selected
    val p = vm.program
    Column(modifier) {
        Row(
            Modifier
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Channel.entries.forEach { c ->
                FilterChip(selected = c == ch, onClick = { vm.select(c) }, label = { Text(p.channelName(c).ifEmpty { c.label }) })
            }
        }
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(ScreenPadding),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            LevelSection(vm, p, ch)
            PeqSection(vm, p, ch)
            if (ch.isInput) GeqSection(vm, p, ch)
            CrossoverSection(vm, p, ch)
            if (ch.isInput) GateSection(vm, p, ch)
            if (!ch.isInput) {
                RoutingSection(vm, p, ch)
                CompressorSection(vm, p, ch)
                LimiterSection(vm, p, ch)
            }
            LinkSection(vm, p, ch)
        }
    }
}

private val msParse: (Double) -> Int = { it.roundToInt() }

@Composable
private fun LevelSection(vm: DspViewModel, p: Program, ch: Channel) {
    SectionCard(stringResource(R.string.section_level, ch.label)) {
        NameField(p.channelName(ch)) { vm.edit(Request.SetChannelName(ch, Names.field(it, Program.NAME_LEN))) }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LevelMeter({ vm.meters?.of(ch) }, Modifier.weight(1f))
            LevelText({ vm.meters?.of(ch) })
        }
        ValueSlider(
            stringResource(R.string.gain), p.gain(ch), 0..Units.GAIN_MAX, Format::gain,
            { vm.edit(Request.SetGain(ch, it)) }, parse = Units::gainRaw,
        )
        SwitchRow(stringResource(R.string.mute), p.isMuted(ch), { vm.edit(Request.SetMute(ch, it)) })
        SwitchRow(stringResource(R.string.polarity), p.polarity(ch) != 0, { vm.edit(Request.SetPolarity(ch, it)) })
        val unit = p.delayUnit
        ValueSlider(
            stringResource(R.string.delay), p.delay(ch), 0..Units.DELAY_MAX, { Format.delay(it, unit) },
            { vm.edit(Request.SetDelay(ch, it)) },
            parse = { v ->
                when (unit) {
                    1 -> Units.delayRaw(v / Units.SPEED_OF_SOUND_M_PER_MS)
                    2 -> Units.delayRaw(v / Units.FEET_PER_METER / Units.SPEED_OF_SOUND_M_PER_MS)
                    else -> Units.delayRaw(v)
                }
            },
        )
        ChoiceRow(
            stringResource(R.string.delay_unit), unit.coerceIn(0, 2), listOf(0, 1, 2),
            { listOf("ms", "m", "ft")[it] }, { vm.edit(Request.SetDelayUnit(it)) },
        )
    }
}

/** Channel name, 8 ASCII characters, sent when the keyboard's Done key is pressed. */
@Composable
private fun NameField(current: String, onSet: (String) -> Unit) {
    var text by remember(current) { mutableStateOf(current) }
    val focus = LocalFocusManager.current
    OutlinedTextField(
        value = text,
        onValueChange = { text = it.take(Program.NAME_LEN) },
        label = { Text(stringResource(R.string.channel_name)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {
            if (text != current) onSet(text)
            focus.clearFocus()
        }),
    )
}

@Composable
private fun PeqSection(vm: DspViewModel, p: Program, ch: Channel) {
    val bypass = p.peqBypassed(ch)
    SectionCard(
        stringResource(R.string.section_peq),
        actions = {
            TextButton(onClick = {
                for (i in 0 until p.peqBandCount(ch)) {
                    val b = p.peq(ch, i)
                    vm.edit(Request.SetPeqBand(ch, i, b.copy(gain = Units.EQ_GAIN_FLAT), p.peqBandBypassed(ch, i)))
                }
            }) { Text(stringResource(R.string.flat)) }
        },
    ) {
        EqGraph(p, ch, vm.selectedBand, vm::selectBand, stringResource(R.string.eq_graph_description, ch.label))
        SwitchRow(stringResource(R.string.peq_bypass), bypass, { vm.edit(Request.SetPeqBypass(ch, it)) })
        val count = p.peqBandCount(ch)
        val band = vm.selectedBand.coerceIn(0, count - 1)
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            for (i in 0 until count) {
                FilterChip(selected = i == band, onClick = { vm.selectBand(i) }, label = { Text("${i + 1}") })
            }
        }
        val b = p.peq(ch, band)
        val bandBypass = p.peqBandBypassed(ch, band)
        fun set(value: dsp206.proto.PeqBand = b, bypassed: Boolean = bandBypass) =
            vm.edit(Request.SetPeqBand(ch, band, value, bypassed))
        ChoiceRow(
            stringResource(R.string.peq_type), b.kind.coerceIn(0, PeqType.entries.size - 1), PeqType.entries.indices.toList(),
            { PeqType.entries[it].label }, { set(b.copy(kind = it)) },
        )
        ValueSlider(
            stringResource(R.string.frequency), b.freq, 0..Units.FREQ_INDEX_MAX, Format::freqIndex,
            { set(b.copy(freq = it)) }, parse = Units::freqIndex,
        )
        ValueSlider(
            stringResource(R.string.gain), b.gain, 0..Units.EQ_GAIN_MAX, Format::eqGain,
            { set(b.copy(gain = it)) }, enabled = PeqType.fromRaw(b.kind)?.hasGain != false, parse = Units::eqGainRaw,
        )
        ValueSlider(
            stringResource(R.string.q), b.q, 0 until Units.Q_TABLE.size, Format::q,
            { set(b.copy(q = it)) }, parse = Units::qIndex,
        )
        SwitchRow(stringResource(R.string.band_bypass, band + 1), bandBypass, { set(bypassed = it) })
    }
}

@Composable
private fun GeqSection(vm: DspViewModel, p: Program, ch: Channel) {
    val bypass = p.geqBypassed(ch)
    SectionCard(
        stringResource(R.string.section_geq),
        actions = {
            TextButton(onClick = {
                for (i in 0 until Program.GEQ_BANDS) vm.edit(Request.SetGeqBand(ch, i, Units.EQ_GAIN_FLAT))
            }) { Text(stringResource(R.string.flat)) }
        },
    ) {
        GeqEditor(
            (0 until Program.GEQ_BANDS).map { p.geq(ch, it) },
            enabled = true,
            onChange = { band, raw -> vm.edit(Request.SetGeqBand(ch, band, raw)) },
            description = stringResource(R.string.geq_description, ch.label),
        )
        Text(stringResource(R.string.geq_hint), style = MaterialTheme.typography.bodySmall)
        SwitchRow(stringResource(R.string.geq_bypass), bypass, { vm.edit(Request.SetGeqBypass(ch, it)) })
    }
}

@Composable
private fun CrossoverSection(vm: DspViewModel, p: Program, ch: Channel) {
    SectionCard(stringResource(R.string.section_crossover)) {
        val hp = p.highPass(ch)
        val lp = p.lowPass(ch)
        CutControls(stringResource(R.string.high_pass), hp) { vm.edit(Request.SetHighPass(ch, it)) }
        CutControls(stringResource(R.string.low_pass), lp) { vm.edit(Request.SetLowPass(ch, it)) }
    }
}

@Composable
private fun CutControls(title: String, f: CutFilter, onChange: (CutFilter) -> Unit) {
    Text(title, style = MaterialTheme.typography.titleSmall)
    ValueSlider(
        stringResource(R.string.frequency), f.freq, 0..Units.FREQ_INDEX_MAX, Format::freqIndex,
        { onChange(f.copy(freq = it)) }, parse = Units::freqIndex,
    )
    ChoiceRow(
        stringResource(R.string.slope), f.slope.coerceIn(0, Units.SLOPES.size - 1), Units.SLOPES.indices.toList(),
        { Units.slopeLabel(it) ?: "?" }, { onChange(f.copy(slope = it)) },
    )
}

@Composable
private fun GateSection(vm: DspViewModel, p: Program, ch: Channel) {
    val g = p.gate(ch)
    fun set(v: Gate) = vm.edit(Request.SetGate(ch, v))
    SectionCard(stringResource(R.string.section_gate)) {
        ValueSlider(
            stringResource(R.string.threshold), g.threshold, 0..Units.GATE_THRESHOLD_MAX, Format::threshold,
            { set(g.copy(threshold = it)) }, parse = { Units.thresholdRaw(it, Units.GATE_THRESHOLD_MAX) },
        )
        ValueSlider(
            stringResource(R.string.attack), g.attack, 0..Units.ATTACK_MAX, Format::time,
            { set(g.copy(attack = it)) }, parse = { Units.timeRaw(msParse(it), 0, Units.ATTACK_MAX) },
        )
        ValueSlider(
            stringResource(R.string.hold), g.hold, Units.HOLD_MIN..Units.HOLD_MAX, Format::time,
            { set(g.copy(hold = it)) }, parse = { Units.timeRaw(msParse(it), Units.HOLD_MIN, Units.HOLD_MAX) },
        )
        ValueSlider(
            stringResource(R.string.release), g.release, Units.RELEASE_MIN..Units.RELEASE_MAX, Format::time,
            { set(g.copy(release = it)) }, parse = { Units.timeRaw(msParse(it), Units.RELEASE_MIN, Units.RELEASE_MAX) },
        )
    }
}

@Composable
private fun RoutingSection(vm: DspViewModel, p: Program, ch: Channel) {
    val routing = p.routing(ch)
    SectionCard(stringResource(R.string.section_routing)) {
        listOf(Channel.IN_A, Channel.IN_B).forEachIndexed { i, input ->
            val on = routing and (1 shl i) != 0
            SwitchRow(
                stringResource(R.string.routing_from, input.label),
                on,
                { vm.edit(Request.SetRouting(ch, if (it) routing or (1 shl i) else routing and (1 shl i).inv())) },
            )
            ValueSlider(
                stringResource(R.string.mix_gain, input.label), p.mix(ch, i), 0..Units.MIX_GAIN_MAX, Format::gain,
                { vm.edit(Request.SetMixGain(ch, i, it)) }, enabled = on, step = 5,
                parse = { Units.gainRaw(it).coerceAtMost(Units.MIX_GAIN_MAX) },
            )
        }
    }
}

@Composable
private fun DynamicsSliders(
    threshold: Int,
    attack: Int,
    release: Int,
    knee: Int,
    onThreshold: (Int) -> Unit,
    onAttack: (Int) -> Unit,
    onRelease: (Int) -> Unit,
    onKnee: (Int) -> Unit,
) {
    ValueSlider(
        stringResource(R.string.threshold), threshold, 0..Units.DYN_THRESHOLD_MAX, Format::threshold,
        onThreshold, parse = { Units.thresholdRaw(it, Units.DYN_THRESHOLD_MAX) },
    )
    ValueSlider(
        stringResource(R.string.attack), attack, 0..Units.ATTACK_MAX, Format::time,
        onAttack, parse = { Units.timeRaw(msParse(it), 0, Units.ATTACK_MAX) },
    )
    ValueSlider(
        stringResource(R.string.release), release, Units.RELEASE_MIN..Units.RELEASE_MAX, Format::time,
        onRelease, parse = { Units.timeRaw(msParse(it), Units.RELEASE_MIN, Units.RELEASE_MAX) },
    )
    ValueSlider(
        stringResource(R.string.knee), knee, 0..Units.KNEE_MAX, { "$it dB" },
        onKnee, parse = { it.roundToInt().coerceIn(0, Units.KNEE_MAX) },
    )
}

@Composable
private fun CompressorSection(vm: DspViewModel, p: Program, ch: Channel) {
    val c = p.compressor(ch)
    fun set(v: Compressor) = vm.edit(Request.SetCompressor(ch, v))
    SectionCard(stringResource(R.string.section_compressor)) {
        ChoiceRow(
            stringResource(R.string.ratio), c.ratio.coerceIn(0, Units.RATIO_TABLE.size - 1), Units.RATIO_TABLE.indices.toList(),
            Format::ratio, { set(c.copy(ratio = it)) },
        )
        DynamicsSliders(
            c.threshold, c.attack, c.release, c.knee,
            { set(c.copy(threshold = it)) }, { set(c.copy(attack = it)) },
            { set(c.copy(release = it)) }, { set(c.copy(knee = it)) },
        )
    }
}

@Composable
private fun LimiterSection(vm: DspViewModel, p: Program, ch: Channel) {
    val l = p.limiter(ch)
    fun set(v: Limiter) = vm.edit(Request.SetLimiter(ch, v))
    SectionCard(stringResource(R.string.section_limiter)) {
        DynamicsSliders(
            l.threshold, l.attack, l.release, l.knee,
            { set(l.copy(threshold = it)) }, { set(l.copy(attack = it)) },
            { set(l.copy(release = it)) }, { set(l.copy(knee = it)) },
        )
    }
}

@Composable
private fun LinkSection(vm: DspViewModel, p: Program, ch: Channel) {
    val linked = p.linkedChannels(ch)
    var copyOpen by remember { mutableStateOf(false) }
    SectionCard(stringResource(R.string.section_link)) {
        Text(stringResource(R.string.link_hint), style = MaterialTheme.typography.bodySmall)
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            ch.domain.filter { it != ch }.forEach { other ->
                FilterChip(
                    selected = other in linked,
                    onClick = { vm.setLinked(ch, other, other !in linked) },
                    label = { Text(other.label) },
                )
            }
        }
        Text(stringResource(R.string.copy_hint, ch.label), style = MaterialTheme.typography.bodySmall)
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (!copyOpen) {
                OutlinedButton(onClick = { copyOpen = true }) { Text(stringResource(R.string.copy_to)) }
            } else {
                ch.domain.filter { it != ch }.forEach { dst ->
                    OutlinedButton(onClick = {
                        vm.copyChannel(ch, dst)
                        copyOpen = false
                    }) { Text(dst.label) }
                }
            }
        }
    }
}

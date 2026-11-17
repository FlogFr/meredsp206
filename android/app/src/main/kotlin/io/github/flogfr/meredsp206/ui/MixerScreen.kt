package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dsp206.editor.Format
import dsp206.proto.Channel
import dsp206.proto.Request
import dsp206.proto.Units
import dsp206.proto.isMuted
import dsp206.proto.linkedChannels
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.R

/** All eight channels at a glance: meter, gain, mute, polarity, routing (design rules). */
@Composable
fun MixerScreen(vm: DspViewModel, modifier: Modifier, onOpenChannel: (Channel) -> Unit) {
    DisposableEffect(vm) {
        vm.setMeterView(true)
        onDispose { vm.setMeterView(false) }
    }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(minSize = 320.dp),
        modifier = modifier,
        contentPadding = ScreenPadding,
        verticalArrangement = Arrangement.spacedBy(12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(Channel.entries) { ch -> ChannelStrip(vm, ch, onOpen = { onOpenChannel(ch) }) }
    }
}

@Composable
private fun ChannelStrip(vm: DspViewModel, ch: Channel, onOpen: () -> Unit) {
    val p = vm.program
    val muted = p.isMuted(ch)
    val linked = p.linkedChannels(ch).size > 1
    OutlinedCard {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(
                    Modifier
                        .weight(1f)
                        .clickable(onClickLabel = stringResource(R.string.mixer_open_editor), onClick = onOpen),
                ) {
                    val name = p.channelName(ch).ifEmpty { ch.label }
                    Text(name, style = MaterialTheme.typography.titleMedium)
                    // The physical channel, when the name does not already say it.
                    val sub = when {
                        linked -> stringResource(R.string.mixer_linked, ch.label)
                        name != ch.label -> ch.label
                        else -> null
                    }
                    sub?.let {
                        Text(it, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                FilterChip(
                    selected = p.polarity(ch) != 0,
                    onClick = { vm.edit(Request.SetPolarity(ch, p.polarity(ch) == 0)) },
                    label = { Text("Ø") },
                    modifier = Modifier.padding(end = 6.dp),
                )
                FilterChip(
                    selected = muted,
                    onClick = { vm.edit(Request.SetMute(ch, !muted)) },
                    label = { Text(stringResource(R.string.mute)) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.error,
                        selectedLabelColor = MaterialTheme.colorScheme.onError,
                    ),
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                LevelMeter({ vm.meters?.of(ch) }, Modifier.weight(1f))
                LevelText({ vm.meters?.of(ch) })
            }
            ValueSlider(
                label = stringResource(R.string.gain),
                value = p.gain(ch),
                range = 0..Units.GAIN_MAX,
                format = Format::gain,
                onChange = { vm.edit(Request.SetGain(ch, it)) },
                step = 5,
                parse = Units::gainRaw,
                compact = true,
            )
            if (!ch.isInput) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.mixer_from), style = MaterialTheme.typography.labelMedium)
                    val routing = p.routing(ch)
                    listOf(Channel.IN_A, Channel.IN_B).forEachIndexed { i, input ->
                        val on = routing and (1 shl i) != 0
                        FilterChip(
                            selected = on,
                            onClick = { vm.edit(Request.SetRouting(ch, routing xor (1 shl i))) },
                            label = { Text(input.label) },
                        )
                    }
                }
            }
        }
    }
}

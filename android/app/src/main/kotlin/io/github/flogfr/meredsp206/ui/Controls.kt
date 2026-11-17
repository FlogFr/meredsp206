package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dsp206.editor.Format
import io.github.flogfr.meredsp206.R
import io.github.flogfr.meredsp206.ui.theme.MeterColors
import kotlin.math.roundToInt

/** A titled card holding one group of settings. */
@Composable
fun SectionCard(
    title: String,
    modifier: Modifier = Modifier,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    ElevatedCard(modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                actions()
            }
            content()
        }
    }
}

/**
 * A raw device value on a slider, with −/+ buttons for single steps. Tapping
 * the value opens a dialog to type it ([parse] turns the typed number, in
 * display units, into a raw value). [compact] puts everything on one row,
 * without the label (it stays available to accessibility services).
 */
@Composable
fun ValueSlider(
    label: String,
    value: Int,
    range: IntRange,
    format: (Int) -> String,
    onChange: (Int) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    step: Int = 1,
    parse: ((Double) -> Int)? = null,
    compact: Boolean = false,
) {
    var typing by remember { mutableStateOf(false) }
    val shown = format(value)
    fun set(v: Int) {
        val c = v.coerceIn(range)
        if (c != value) onChange(c)
    }
    Column(modifier.fillMaxWidth()) {
        if (!compact) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = { typing = true }, enabled = enabled && parse != null) { Text(shown) }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            StepButton("−", stringResource(R.string.decrease, label), enabled) { set(value - step) }
            Slider(
                value = value.toFloat(),
                onValueChange = { set(it.roundToInt()) },
                valueRange = range.first.toFloat()..range.last.toFloat(),
                enabled = enabled,
                modifier = Modifier
                    .weight(1f)
                    .semantics {
                        contentDescription = label
                        stateDescription = shown
                    },
            )
            StepButton("+", stringResource(R.string.increase, label), enabled) { set(value + step) }
            if (compact) {
                TextButton(onClick = { typing = true }, enabled = enabled && parse != null) { Text(shown) }
            }
        }
    }
    if (typing && parse != null) {
        NumberDialog(label, shown, onDismiss = { typing = false }) { number ->
            set(parse(number))
            typing = false
        }
    }
}

@Composable
private fun StepButton(text: String, description: String, enabled: Boolean, onClick: () -> Unit) {
    FilledTonalIconButton(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier
            .size(36.dp)
            .semantics { contentDescription = description },
    ) {
        Text(text, style = MaterialTheme.typography.titleMedium)
    }
}

/** Asks for a number; accepts "1k", "-3 dB", "2,5" (Format.parseNumber). */
@Composable
fun NumberDialog(title: String, initial: String, onDismiss: () -> Unit, onValue: (Double) -> Unit) {
    var text by remember { mutableStateOf(initial) }
    val number = Format.parseNumber(text)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                singleLine = true,
                isError = number == null,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text),
            )
        },
        confirmButton = {
            TextButton(onClick = { number?.let(onValue) }, enabled = number != null) { Text(stringResource(R.string.ok)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** A labelled switch row. */
@Composable
fun SwitchRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        Switch(
            checked = checked,
            onCheckedChange = onChange,
            enabled = enabled,
            modifier = Modifier.semantics { contentDescription = label },
        )
    }
}

/** A labelled choice among [options], shown as a button with a menu. */
@Composable
fun <T> ChoiceRow(
    label: String,
    selected: T,
    options: List<T>,
    optionLabel: (T) -> String,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    var open by remember { mutableStateOf(false) }
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        Box {
            OutlinedButton(onClick = { open = true }, enabled = enabled) {
                Text(optionLabel(selected))
                Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                options.forEach { option ->
                    DropdownMenuItem(
                        text = { Text(optionLabel(option)) },
                        onClick = {
                            open = false
                            if (option != selected) onSelect(option)
                        },
                    )
                }
            }
        }
    }
}

/** Meter scale in dBu (as the GTK app: −60…+18). */
private const val METER_MIN = -60.0
private const val METER_MAX = 18.0

private fun meterStop(db: Double) = ((db - METER_MIN) / (METER_MAX - METER_MIN)).toFloat()

/**
 * Horizontal level meter. [level] is read while drawing, so a new reading
 * only redraws the bar instead of recomposing the screen.
 */
@Composable
fun LevelMeter(level: () -> Double?, modifier: Modifier = Modifier) {
    val track = MaterialTheme.colorScheme.surfaceVariant
    val tick = MaterialTheme.colorScheme.onSurfaceVariant
    Canvas(
        modifier
            .fillMaxWidth()
            .height(10.dp),
    ) {
        drawRect(track)
        val db = level()
        if (db != null) {
            val frac = meterStop(db).coerceIn(0f, 1f)
            drawRect(
                Brush.horizontalGradient(
                    0f to MeterColors.ok,
                    meterStop(0.0) to MeterColors.ok,
                    meterStop(8.0) to MeterColors.hot,
                    meterStop(15.0) to MeterColors.clip,
                    1f to MeterColors.clip,
                    startX = 0f,
                    endX = size.width,
                ),
                size = Size(size.width * frac, size.height),
            )
        }
        val x0 = size.width * meterStop(0.0)
        drawRect(tick, topLeft = Offset(x0, 0f), size = Size(1.dp.toPx(), size.height))
    }
}

/** dBu reading next to a meter. */
@Composable
fun LevelText(level: () -> Double?, modifier: Modifier = Modifier) {
    val db = level()
    Text(
        if (db == null) "" else Format.dbu(db) + " dBu",
        style = MaterialTheme.typography.labelSmall,
        color = if (db != null && db >= 15.0) MeterColors.clip else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

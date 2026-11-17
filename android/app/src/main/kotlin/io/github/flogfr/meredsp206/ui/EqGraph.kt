package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dsp206.editor.Format
import dsp206.editor.Response
import dsp206.proto.Channel
import dsp206.proto.PeqType
import dsp206.proto.Program
import dsp206.proto.Units
import dsp206.proto.peqBandBypassed
import kotlin.math.abs
import kotlin.math.log10
import kotlin.math.roundToInt

/** Vertical range of the EQ graph, ±18 dB (as the GTK app). */
private const val GRAPH_DB = 18.0
private val AXIS = Response.freqAxis(160)
private val GRID_HZ = listOf(20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0)

private fun xOf(f: Double, w: Float) = (log10(f / 20.0) / 3.0 * w).toFloat()

private fun yOf(db: Double, h: Float): Float {
    val pad = 10f
    return (h / 2 - db / GRAPH_DB * (h / 2 - pad)).toFloat()
}

/**
 * Frequency response of [ch] (crossover, PEQ, GEQ) with one handle per PEQ
 * band. Tapping near a handle selects that band.
 */
@Composable
fun EqGraph(program: Program, ch: Channel, selectedBand: Int, onSelectBand: (Int) -> Unit, description: String) {
    val curve = remember(program, ch) { Response.channelDb(program, ch, AXIS) }
    val handles = remember(program, ch) {
        (0 until program.peqBandCount(ch)).map { i ->
            val band = program.peq(ch, i)
            val gain = if (PeqType.fromRaw(band.kind)?.hasGain == true) Units.eqGainDb(band.gain) else 0.0
            Triple(Units.freqHz(band.freq), gain, program.peqBandBypassed(ch, i))
        }
    }
    val select by rememberUpdatedState(onSelectBand)
    val colors = MaterialTheme.colorScheme
    val measurer = rememberTextMeasurer()
    val labelStyle = TextStyle(fontSize = 10.sp, color = colors.onSurfaceVariant)
    Canvas(
        Modifier
            .fillMaxWidth()
            .height(200.dp)
            .semantics { contentDescription = description }
            .pointerInput(handles) {
                detectTapGestures { pos ->
                    val w = size.width.toFloat()
                    val h = size.height.toFloat()
                    val nearest = handles.indices.minByOrNull { i ->
                        val (f, g, _) = handles[i]
                        val dx = xOf(f, w) - pos.x
                        val dy = yOf(g, h) - pos.y
                        dx * dx + dy * dy
                    }
                    if (nearest != null) select(nearest)
                }
            },
    ) {
        val w = size.width
        val h = size.height
        drawRect(colors.surfaceVariant.copy(alpha = 0.4f))
        for (f in GRID_HZ) {
            val x = xOf(f, w)
            drawLine(colors.outlineVariant, Offset(x, 0f), Offset(x, h), strokeWidth = 1f)
            val label = if (f >= 1000) "${(f / 1000).roundToInt()}k" else "${f.roundToInt()}"
            drawText(measurer, label, Offset(x + 3f, h - 14.dp.toPx()), labelStyle)
        }
        for (db in listOf(-18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0)) {
            val y = yOf(db, h)
            drawLine(if (db == 0.0) colors.outline else colors.outlineVariant, Offset(0f, y), Offset(w, y), strokeWidth = 1f)
            if (abs(db) < 18.0) drawText(measurer, "%+.0f".format(db), Offset(4f, y - 14.dp.toPx()), labelStyle)
        }
        // Shade what the crossover cuts away.
        val hp = xOf(Units.freqHz(program.highPass(ch).freq), w).coerceAtLeast(0f)
        val lp = xOf(Units.freqHz(program.lowPass(ch).freq), w).coerceAtMost(w)
        drawRect(colors.primary.copy(alpha = 0.08f), size = Size(hp, h))
        drawRect(colors.primary.copy(alpha = 0.08f), topLeft = Offset(lp, 0f), size = Size(w - lp, h))
        drawCurve(curve, colors.primary)
        handles.forEachIndexed { i, (f, g, bypassed) ->
            val c = Offset(xOf(f, w), yOf(g, h))
            val color = when {
                i == selectedBand -> colors.tertiary
                bypassed -> colors.outline
                else -> colors.primary
            }
            drawCircle(color, radius = if (i == selectedBand) 9.dp.toPx() else 6.dp.toPx(), center = c)
            drawText(measurer, "${i + 1}", c + Offset(-3.dp.toPx(), -22.dp.toPx()), labelStyle)
        }
    }
}

private fun DrawScope.drawCurve(curve: DoubleArray, color: androidx.compose.ui.graphics.Color) {
    val path = Path()
    curve.forEachIndexed { i, db ->
        val x = xOf(AXIS[i], size.width)
        val y = yOf(db.coerceIn(-40.0, 40.0), size.height)
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    drawPath(path, color, style = Stroke(width = 2.dp.toPx()))
}

/**
 * The 31-band graphic EQ of an input: drag or tap on the bars to set a band.
 * [onChange] gets the band and its raw gain (0..240, 120 = 0 dB).
 */
@Composable
fun GeqEditor(gains: List<Int>, enabled: Boolean, onChange: (Int, Int) -> Unit, description: String) {
    var touched by remember { mutableStateOf<Int?>(null) }
    val change by rememberUpdatedState(onChange)
    val current by rememberUpdatedState(gains)
    val colors = MaterialTheme.colorScheme
    Column {
        val band = touched
        Text(
            if (band == null) " " else Format.hz(Units.GEQ_BANDS_HZ[band]) + "  " + Format.eqGain(gains[band]),
            style = MaterialTheme.typography.labelMedium,
        )
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(160.dp)
                .semantics { contentDescription = description }
                .pointerInput(enabled) {
                    if (!enabled) return@pointerInput
                    awaitEachGesture {
                        val down = awaitFirstDown()
                        fun apply(p: Offset) {
                            val b = (p.x / size.width * gains.size).toInt().coerceIn(0, gains.size - 1)
                            val raw = ((1f - p.y / size.height) * Units.EQ_GAIN_MAX).roundToInt().coerceIn(0, Units.EQ_GAIN_MAX)
                            touched = b
                            if (current[b] != raw) change(b, raw)
                        }
                        apply(down.position)
                        down.consume()
                        while (true) {
                            val event = awaitPointerEvent()
                            val c = event.changes.firstOrNull { it.id == down.id } ?: break
                            if (!c.pressed) break
                            apply(c.position)
                            c.consume()
                        }
                    }
                },
        ) {
            val n = gains.size
            val bw = size.width / n
            val mid = size.height / 2
            drawLine(colors.outline, Offset(0f, mid), Offset(size.width, mid), strokeWidth = 1f)
            gains.forEachIndexed { i, raw ->
                val y = (1f - raw.toFloat() / Units.EQ_GAIN_MAX) * size.height
                val top = minOf(y, mid)
                val color = when {
                    !enabled -> colors.outline
                    i == touched -> colors.tertiary
                    else -> colors.primary
                }
                drawRect(color, topLeft = Offset(i * bw + bw * 0.15f, top), size = Size(bw * 0.7f, maxOf(abs(y - mid), 2f)))
            }
        }
    }
}

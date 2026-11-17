package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.R

/** Every frame sent (→) and received (←), as in the GTK app's log. */
@Composable
fun LogScreen(vm: DspViewModel, modifier: Modifier) {
    val version = vm.logVersion
    val lines = remember(version) { vm.log() }
    val state = rememberLazyListState()
    LaunchedEffect(lines.size) {
        if (lines.isNotEmpty()) state.scrollToItem(lines.size - 1)
    }
    Column(modifier.padding(horizontal = 8.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                stringResource(R.string.log_title),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier
                    .weight(1f)
                    .padding(8.dp),
            )
            OutlinedButton(onClick = vm::clearLog) { Text(stringResource(R.string.log_clear)) }
        }
        LazyColumn(state = state) {
            items(lines) { line ->
                Text(line, fontFamily = FontFamily.Monospace, fontSize = 10.sp, softWrap = false)
            }
        }
    }
}

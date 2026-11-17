package io.github.flogfr.meredsp206.ui

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dsp206.proto.Program
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.R
import java.io.IOException

private sealed class PresetDialog {
    data class Recall(val n: Int) : PresetDialog()

    data class Store(val n: Int) : PresetDialog()

    data object Push : PresetDialog()
}

/** Presets on the unit (F00, U01–U20) and program files on the phone. */
@Composable
fun PresetsScreen(vm: DspViewModel, modifier: Modifier) {
    val online = vm.status.kind.isOnline
    val info = vm.info
    val context = LocalContext.current
    var dialog by remember { mutableStateOf<PresetDialog?>(null) }

    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        if (uri != null) {
            try {
                context.contentResolver.openOutputStream(uri)?.use { it.write(vm.programFile()) }
                vm.fileSaved()
            } catch (_: IOException) {
                vm.loadProgramFile(ByteArray(0)) // reports an invalid file
            }
        }
    }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            val bytes = try {
                context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            } catch (_: IOException) {
                null
            }
            vm.loadProgramFile(bytes ?: ByteArray(0))
        }
    }

    LazyColumn(modifier, contentPadding = ScreenPadding, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        item {
            SectionCard(stringResource(R.string.presets_current)) {
                Text(vm.program.nameText, style = MaterialTheme.typography.titleLarge)
                info.currentPreset?.let { Text(stringResource(R.string.presets_loaded_from, presetLabel(it))) }
                if (!online) Text(stringResource(R.string.presets_offline), style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { save.launch(fileName(vm.program)) }) { Text(stringResource(R.string.file_save)) }
                    OutlinedButton(onClick = { open.launch(arrayOf("*/*")) }) { Text(stringResource(R.string.file_open)) }
                }
                if (online) {
                    TextButton(onClick = { dialog = PresetDialog.Push }) { Text(stringResource(R.string.push_program)) }
                }
            }
        }
        items((0..20).toList()) { n ->
            val name = info.presetNames.getOrNull(n).orEmpty()
            val current = info.currentPreset == n
            ListItem(
                headlineContent = { Text(name.ifEmpty { "—" }) },
                overlineContent = {
                    Text(
                        presetLabel(n) + if (n > 0 && n in info.usedSlots) " · " + stringResource(R.string.presets_used) else "",
                    )
                },
                supportingContent = if (current) {
                    { Text(stringResource(R.string.presets_current_marker)) }
                } else {
                    null
                },
                colors = if (current) {
                    ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.secondaryContainer)
                } else {
                    ListItemDefaults.colors()
                },
                trailingContent = {
                    Row {
                        TextButton(onClick = { dialog = PresetDialog.Recall(n) }, enabled = online) {
                            Text(stringResource(R.string.preset_load))
                        }
                        if (n > 0) {
                            TextButton(onClick = { dialog = PresetDialog.Store(n) }, enabled = online) {
                                Text(stringResource(R.string.preset_store))
                            }
                        }
                    }
                },
            )
        }
    }

    when (val d = dialog) {
        is PresetDialog.Recall -> ConfirmDialog(
            title = stringResource(R.string.preset_load_title, presetLabel(d.n), info.presetNames.getOrNull(d.n).orEmpty()),
            text = stringResource(R.string.preset_load_text),
            confirm = stringResource(R.string.preset_load),
            onDismiss = { dialog = null },
        ) {
            vm.recallPreset(d.n)
            dialog = null
        }
        is PresetDialog.Store -> StoreDialog(d.n, vm.program.nameText, info.presetNames.getOrNull(d.n).orEmpty(), { dialog = null }) { name ->
            vm.storePreset(d.n, name)
            dialog = null
        }
        PresetDialog.Push -> ConfirmDialog(
            title = stringResource(R.string.push_program),
            text = stringResource(R.string.push_program_text),
            confirm = stringResource(R.string.send),
            onDismiss = { dialog = null },
        ) {
            vm.pushProgram()
            dialog = null
        }
        null -> {}
    }
}

private fun fileName(p: Program): String {
    val base = p.nameText.ifBlank { "program" }.replace(Regex("[^A-Za-z0-9 _-]"), "_").trim()
    return "$base.dsp206"
}

@Composable
fun ConfirmDialog(title: String, text: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = { TextButton(onClick = onConfirm) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Name the current program and store it into U[n]; the old content of the slot is lost. */
@Composable
private fun StoreDialog(n: Int, currentName: String, slotName: String, onDismiss: () -> Unit, onStore: (String) -> Unit) {
    var name by remember { mutableStateOf(currentName.take(Program.PROGRAM_NAME_LEN)) }
    val valid = name.isNotBlank() && name.all { it.code in 0x20..0x7e }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.preset_store_title, presetLabel(n))) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.preset_store_text, presetLabel(n), slotName))
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it.take(Program.PROGRAM_NAME_LEN) },
                    label = { Text(stringResource(R.string.preset_name)) },
                    supportingText = { Text(stringResource(R.string.preset_name_hint)) },
                    isError = !valid,
                    singleLine = true,
                )
            }
        },
        confirmButton = { TextButton(onClick = { onStore(name) }, enabled = valid) { Text(stringResource(R.string.preset_store)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

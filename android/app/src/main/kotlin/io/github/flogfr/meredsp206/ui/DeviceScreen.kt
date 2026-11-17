package io.github.flogfr.meredsp206.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dsp206.editor.LinkKind
import dsp206.proto.Program
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.R

/** Connection: status, Wi-Fi (discovery or address), USB, demo, protocol log. */
@Composable
fun DeviceScreen(vm: DspViewModel, modifier: Modifier, onShowLog: () -> Unit) {
    val s = vm.status
    val online = s.kind.isOnline
    LazyColumn(modifier, contentPadding = ScreenPadding, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            SectionCard(stringResource(R.string.device_status)) {
                Text(statusText(s), style = MaterialTheme.typography.bodyLarge)
                vm.info.identity?.let { Text(stringResource(R.string.device_identity, it)) }
                vm.info.currentPreset?.let { Text(stringResource(R.string.device_current_preset, presetLabel(it), vm.program.nameText)) }
                if (s.kind == LinkKind.CONNECTING) CircularProgressIndicator(Modifier.size(24.dp))
                if (online && s.pagesReceived < Program.PAGE_COUNT && s.missingPages.isEmpty()) {
                    Text(stringResource(R.string.device_reading, s.pagesReceived, Program.PAGE_COUNT))
                }
                if (s.unconfirmed > 0 || s.timeouts > 0) {
                    Text(
                        stringResource(R.string.device_counters, s.unconfirmed, s.timeouts),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                if (s.missingPages.isNotEmpty()) {
                    Text(stringResource(R.string.device_missing_pages, s.missingPages.size), color = MaterialTheme.colorScheme.error)
                }
                s.reconnectWait?.let { Text(stringResource(R.string.device_reconnect_wait, it)) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (online) {
                        OutlinedButton(onClick = vm::readAgain) { Text(stringResource(R.string.device_read_again)) }
                    }
                    if (s.missingPages.isNotEmpty()) {
                        OutlinedButton(onClick = vm::retryMissingPages) { Text(stringResource(R.string.device_retry_pages)) }
                    }
                    if (online || s.kind == LinkKind.CONNECTING) {
                        Button(onClick = vm::disconnect) { Text(stringResource(R.string.device_disconnect)) }
                    }
                }
            }
        }
        item { NetworkCard(vm) }
        item {
            SectionCard(stringResource(R.string.usb_title)) {
                Text(
                    stringResource(if (vm.usbPresent) R.string.usb_present else R.string.usb_absent),
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(stringResource(R.string.usb_hint), style = MaterialTheme.typography.bodySmall)
                Button(onClick = vm::connectUsb, enabled = vm.usbPresent) { Text(stringResource(R.string.usb_connect)) }
            }
        }
        item {
            SectionCard(stringResource(R.string.demo_title)) {
                Text(stringResource(R.string.demo_hint), style = MaterialTheme.typography.bodySmall)
                OutlinedButton(onClick = vm::startDemo) { Text(stringResource(R.string.demo_start)) }
            }
        }
        item {
            SectionCard(stringResource(R.string.advanced_title)) {
                var idText by rememberSaveable(vm.deviceId) { mutableStateOf(vm.deviceId.toString()) }
                OutlinedTextField(
                    value = idText,
                    onValueChange = { t ->
                        idText = t.filter { it.isDigit() }.take(3)
                        idText.toIntOrNull()?.takeIf { it in 1..254 }?.let(vm::updateDeviceId)
                    },
                    label = { Text(stringResource(R.string.device_id)) },
                    supportingText = { Text(stringResource(R.string.device_id_hint)) },
                    isError = idText.toIntOrNull() !in 1..254,
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                )
                TextButton(onClick = onShowLog) { Text(stringResource(R.string.log_open)) }
            }
        }
    }
}

@Composable
private fun NetworkCard(vm: DspViewModel) {
    SectionCard(stringResource(R.string.network_title)) {
        val local = vm.localAddress
        Text(
            if (local == null) stringResource(R.string.network_no_wifi) else stringResource(R.string.network_phone_address, local.toString()),
            style = MaterialTheme.typography.bodyMedium,
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = vm::discover, enabled = !vm.discovering) { Text(stringResource(R.string.network_search)) }
            if (vm.discovering) CircularProgressIndicator(Modifier.size(24.dp))
        }
        vm.found.forEach { d ->
            val ip = d.address.hostAddress ?: ""
            ListItem(
                headlineContent = { Text(ip) },
                supportingContent = { Text((listOf(d.name) + d.details).joinToString(" · ")) },
                trailingContent = {
                    TextButton(onClick = { vm.connectNetwork(ip) }) { Text(stringResource(R.string.network_connect)) }
                },
            )
        }
        if (!vm.discovering && vm.found.isEmpty()) {
            Text(stringResource(R.string.network_search_hint), style = MaterialTheme.typography.bodySmall)
        }
        OutlinedTextField(
            value = vm.address,
            onValueChange = vm::updateAddress,
            label = { Text(stringResource(R.string.network_address)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { vm.connectNetwork() }),
        )
        Button(onClick = { vm.connectNetwork() }) { Text(stringResource(R.string.network_connect)) }
        Text(stringResource(R.string.network_warning), style = MaterialTheme.typography.bodySmall)
    }
}

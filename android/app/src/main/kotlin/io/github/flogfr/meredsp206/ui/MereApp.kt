package io.github.flogfr.meredsp206.ui

import android.content.res.Resources
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleStartEffect
import dsp206.editor.EditorModel
import dsp206.editor.LinkKind
import dsp206.editor.Notice
import dsp206.editor.UiNotice
import dsp206.link.CloseReason
import io.github.flogfr.meredsp206.AppNotice
import io.github.flogfr.meredsp206.DspViewModel
import io.github.flogfr.meredsp206.LinkStatus
import io.github.flogfr.meredsp206.R

enum class Screen { DEVICE, MIXER, CHANNEL, PRESETS, LOG }

/** The whole app: top bar with status and undo, bottom navigation, snackbars. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MereApp(vm: DspViewModel) {
    var screen by rememberSaveable { mutableStateOf(Screen.DEVICE) }
    val snackbar = remember { SnackbarHostState() }
    val resources = LocalResources.current

    LifecycleStartEffect(vm) {
        vm.setForeground(true)
        vm.refreshUsb()
        onStopOrDispose { vm.setForeground(false) }
    }
    LaunchedEffect(vm) {
        vm.notices.collect { n ->
            if (n is AppNotice.Connected) screen = Screen.MIXER
            snackbar.showSnackbar(noticeText(resources, n))
        }
    }
    BackHandler(enabled = screen == Screen.LOG) { screen = Screen.DEVICE }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(stringResource(R.string.app_name))
                        Text(
                            statusText(vm.status),
                            style = MaterialTheme.typography.labelMedium,
                            color = if (vm.status.healthSilent) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    IconButton(onClick = vm::undo, enabled = vm.info.canUndo) {
                        Icon(ImageVector.vectorResource(R.drawable.ic_undo), stringResource(R.string.undo))
                    }
                    IconButton(onClick = vm::redo, enabled = vm.info.canRedo) {
                        Icon(ImageVector.vectorResource(R.drawable.ic_redo), stringResource(R.string.redo))
                    }
                },
            )
        },
        bottomBar = {
            NavigationBar {
                NavItem(screen == Screen.DEVICE || screen == Screen.LOG, R.string.tab_device, { Icon(painterResource(R.drawable.ic_wifi), null) }) {
                    screen = Screen.DEVICE
                }
                NavItem(screen == Screen.MIXER, R.string.tab_mixer, { Icon(painterResource(R.drawable.ic_tune), null) }) {
                    screen = Screen.MIXER
                }
                NavItem(screen == Screen.CHANNEL, R.string.tab_channel, { Icon(painterResource(R.drawable.ic_equalizer), null) }) {
                    screen = Screen.CHANNEL
                }
                NavItem(screen == Screen.PRESETS, R.string.tab_presets, { Icon(painterResource(R.drawable.ic_bookmark), null) }) {
                    screen = Screen.PRESETS
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        val inner = Modifier.padding(padding)
        when (screen) {
            Screen.DEVICE -> DeviceScreen(vm, inner, onShowLog = { screen = Screen.LOG })
            Screen.MIXER -> MixerScreen(vm, inner, onOpenChannel = {
                vm.select(it)
                screen = Screen.CHANNEL
            })
            Screen.CHANNEL -> ChannelScreen(vm, inner)
            Screen.PRESETS -> PresetsScreen(vm, inner)
            Screen.LOG -> LogScreen(vm, inner)
        }
    }
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.NavItem(
    selected: Boolean,
    label: Int,
    icon: @Composable () -> Unit,
    onClick: () -> Unit,
) {
    NavigationBarItem(selected = selected, onClick = onClick, icon = icon, label = { Text(stringResource(label)) })
}

/** Content padding for scrolling screens. */
val ScreenPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)

@Composable
fun statusText(s: LinkStatus): String = when (s.kind) {
    LinkKind.OFFLINE -> stringResource(R.string.status_offline)
    LinkKind.CONNECTING -> stringResource(R.string.status_connecting, s.target)
    LinkKind.NETWORK, LinkKind.USB, LinkKind.DEMO -> {
        val where = when (s.kind) {
            LinkKind.NETWORK -> stringResource(R.string.status_network, s.target)
            LinkKind.USB -> stringResource(R.string.status_usb)
            else -> stringResource(R.string.status_demo)
        }
        if (s.healthSilent) stringResource(R.string.status_silent, where) else where
    }
}

fun presetLabel(n: Int) = EditorModel.presetLabel(n)

/** The text of a snackbar. */
fun noticeText(r: Resources, n: AppNotice): String = when (n) {
    is AppNotice.Connected -> r.getString(
        when (n.kind) {
            LinkKind.USB -> R.string.notice_connected_usb
            LinkKind.DEMO -> R.string.notice_connected_demo
            else -> R.string.notice_connected_network
        },
    )
    is AppNotice.ConnectFailed -> r.getString(R.string.notice_connect_failed, n.detail ?: "")
    is AppNotice.NotAnAddress -> r.getString(R.string.notice_not_an_address, n.text)
    is AppNotice.ReconnectTooSoon -> r.getString(R.string.notice_reconnect_soon, n.seconds)
    AppNotice.NoUsbDevice -> r.getString(R.string.notice_no_usb)
    AppNotice.UsbPermissionDenied -> r.getString(R.string.notice_usb_denied)
    AppNotice.NotConnected -> r.getString(R.string.notice_not_connected)
    is AppNotice.Recalling -> r.getString(R.string.notice_recalling, presetLabel(n.n))
    is AppNotice.Storing -> r.getString(R.string.notice_storing, presetLabel(n.n), n.name)
    is AppNotice.Sending -> r.getQuantityString(R.plurals.notice_sending, n.count, n.count)
    AppNotice.FileSaved -> r.getString(R.string.notice_file_saved)
    AppNotice.FileInvalid -> r.getString(R.string.notice_file_invalid)
    is AppNotice.Copied -> r.getString(R.string.notice_copied, n.src.label, n.dst.label)
    is AppNotice.FromSession -> when (val s = n.notice) {
        UiNotice.ProgramRead -> r.getString(R.string.notice_program_read)
        is UiNotice.ProgramIncomplete -> r.getString(R.string.notice_program_incomplete, s.missing)
        UiNotice.DeviceSilent -> r.getString(R.string.notice_silent)
        is UiNotice.Disconnected -> when (s.reason) {
            CloseReason.CLOSED -> r.getString(R.string.notice_disconnected)
            CloseReason.REMOTE_CLOSED -> r.getString(R.string.notice_remote_closed)
            CloseReason.SEND_FAILED, CloseReason.RECEIVE_FAILED -> r.getString(R.string.notice_link_error, s.detail ?: "")
        }
        is UiNotice.Preset -> when (val p = s.notice) {
            is Notice.Stored -> r.getString(R.string.notice_stored, presetLabel(p.n), p.name)
            is Notice.NotStored -> r.getString(R.string.notice_not_stored, presetLabel(p.n), p.actual)
            is Notice.StoreUnconfirmed -> r.getString(R.string.notice_store_unconfirmed, presetLabel(p.n))
            is Notice.RecallUnconfirmed -> r.getString(R.string.notice_recall_unconfirmed, presetLabel(p.n))
        }
    }
}

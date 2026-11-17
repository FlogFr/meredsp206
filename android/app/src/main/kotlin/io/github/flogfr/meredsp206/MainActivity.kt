package io.github.flogfr.meredsp206

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import io.github.flogfr.meredsp206.ui.MereApp
import io.github.flogfr.meredsp206.ui.theme.MereTheme

class MainActivity : ComponentActivity() {
    private val vm: DspViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MereTheme {
                MereApp(vm)
            }
        }
    }

    /** The app is already open and a DSP 206 was plugged in (USB_DEVICE_ATTACHED). */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        vm.refreshUsb()
    }
}

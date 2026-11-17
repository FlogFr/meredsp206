package io.github.flogfr.meredsp206.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val Dark = darkColorScheme(
    primary = Color(0xFF78C2FF),
    onPrimary = Color(0xFF00325A),
    primaryContainer = Color(0xFF004A80),
    onPrimaryContainer = Color(0xFFD2E4FF),
    secondary = Color(0xFF3DDC84),
    background = Color(0xFF111418),
    surface = Color(0xFF111418),
)

private val Light = lightColorScheme(
    primary = Color(0xFF0061A4),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFD2E4FF),
    onPrimaryContainer = Color(0xFF001D36),
    secondary = Color(0xFF006D3B),
)

/** Level meter colours (design rules: green, amber near the top, red for clipping). */
object MeterColors {
    val ok = Color(0xFF33D17A)
    val hot = Color(0xFFF5D42E)
    val clip = Color(0xFFE01B24)
}

/** Follows the system light/dark setting; uses the wallpaper colours on Android 12 and later. */
@Composable
fun MereTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val scheme = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        val context = LocalContext.current
        if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    } else if (dark) {
        Dark
    } else {
        Light
    }
    MaterialTheme(colorScheme = scheme, content = content)
}

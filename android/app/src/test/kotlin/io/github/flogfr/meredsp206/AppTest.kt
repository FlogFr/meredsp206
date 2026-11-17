package io.github.flogfr.meredsp206

import android.graphics.Bitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import dsp206.editor.LinkKind
import dsp206.proto.Channel
import dsp206.proto.isMuted
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

/**
 * The whole app on the JVM (Robolectric) against the simulated unit: every
 * screen renders, and taps reach the device. Screenshots of each screen go to
 * app/build/screenshots/ (screen size of a 6.7" phone such as the Moto G86).
 */
@RunWith(AndroidJUnit4::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-xxhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@OptIn(ExperimentalTestApi::class)
class AppTest {
    @get:Rule
    val compose = createAndroidComposeRule<MainActivity>()

    private val vm get() = compose.activity.viewModels()

    private fun MainActivity.viewModels(): DspViewModel =
        androidx.lifecycle.ViewModelProvider(this)[DspViewModel::class.java]

    private fun shot(name: String) {
        compose.waitForIdle()
        val dir = File("build/screenshots").apply { mkdirs() }
        val bitmap = compose.onRoot().captureToImage().asAndroidBitmap()
        File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private fun startDemo() {
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText("Start the demo"))
        compose.onNodeWithText("Start the demo").performClick()
        compose.waitUntil(10_000) { vm.status.kind == LinkKind.DEMO && vm.info.presetNames.size == 21 && vm.status.pagesReceived >= 19 }
    }

    @Test
    fun launcherIconIsAdaptiveWithAThemedLayer() {
        val icon = androidx.core.content.ContextCompat.getDrawable(compose.activity, R.mipmap.ic_launcher)
        assertTrue(icon is android.graphics.drawable.AdaptiveIconDrawable)
        val adaptive = icon as android.graphics.drawable.AdaptiveIconDrawable
        assertTrue(adaptive.monochrome != null)
        // Draw it the way a launcher does, for a look at the result.
        val bitmap = Bitmap.createBitmap(432, 432, Bitmap.Config.ARGB_8888)
        adaptive.setBounds(0, 0, 432, 432)
        adaptive.draw(android.graphics.Canvas(bitmap))
        val dir = File("build/screenshots").apply { mkdirs() }
        File(dir, "0-launcher-icon.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        // The background gradient fills the corners and the motif the centre.
        assertTrue(bitmap.getPixel(216, 216) != bitmap.getPixel(2, 2))
    }

    @Test
    fun deviceScreenOffline() {
        assertEquals(2, compose.onAllNodesWithText("Offline").fetchSemanticsNodes().size) // top bar and status card
        compose.onNodeWithText("Network (Wi-Fi)").assertExists()
        shot("1-device")
    }

    @Test
    fun demoMixerChannelPresetsAndLog() {
        startDemo()
        // Connecting switches to the mixer (a lazy grid: only the first strips are composed).
        compose.waitUntil(5_000) { compose.onAllNodesWithText("InB").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithText("Demo (simulated unit)").assertExists()
        compose.waitUntil(5_000) { vm.meters != null }
        shot("2-mixer")

        // Mute InA from the mixer: the simulated unit gets it.
        compose.onAllNodesWithText("Mute")[0].performClick()
        compose.waitUntil(5_000) { vm.program.isMuted(Channel.IN_A) }
        assertTrue(vm.program.isMuted(Channel.IN_A))

        compose.onNodeWithText("Channel").performClick()
        compose.onNodeWithText("Parametric EQ").assertExists()
        shot("3-channel")
        compose.onNodeWithText("Noise gate").performScrollTo()
        shot("4-channel-gate")

        compose.onNodeWithText("Presets").performClick()
        compose.waitUntil(5_000) { compose.onAllNodes(hasText("U01")).fetchSemanticsNodes().isNotEmpty() }
        shot("5-presets")

        compose.onNodeWithText("Device").performClick()
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText("Protocol log"))
        compose.onNodeWithText("Protocol log").performClick()
        compose.waitUntil(5_000) { vm.log().isNotEmpty() }
        shot("6-log")

        // Undo brings InA back and sends the unmute.
        assertTrue(vm.info.canUndo)
        vm.undo()
        compose.waitForIdle()
        assertEquals(false, vm.program.isMuted(Channel.IN_A))
    }
}

package dsp206.proto

import dsp206.repoFile
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Keeps this Kotlin port in agreement with the documentation, the safety rule
 * and the Rust crate it mirrors (claude/rules/testing.md). A command added in
 * Rust must be added here too, and the other way round.
 */
class DocsSyncTest {
    private fun code(c: Int) = "`0x%02x`".format(c)

    /** The hex codes of a `const NAME: [u8; N] = [...]` array in the Rust source. */
    private fun rustCodes(name: String): List<Int> {
        val src = repoFile("crates/dsp206-proto/src/command.rs").readText()
        val start = src.indexOf("pub const $name:")
        assertTrue(start >= 0, "$name not found in command.rs")
        val body = src.substring(src.indexOf("= [", start) + 3, src.indexOf("];", start))
        return Regex("0x([0-9A-Fa-f]{2})").findAll(body).map { it.groupValues[1].toInt(16) }.toList()
    }

    @Test
    fun everyImplementedCommandIsDocumented() {
        val doc = repoFile("claude/docs/protocol.md").readText().lowercase()
        for (c in Codes.ALL) assertTrue(code(c) in doc, "${code(c)} is implemented but missing from protocol.md")
    }

    @Test
    fun commandCodesMatchTheRustCrate() {
        assertEquals(rustCodes("ALL_CODES"), Codes.ALL)
        assertEquals(rustCodes("READ_ONLY_CODES").sorted(), Codes.READ_ONLY.sorted())
    }

    @Test
    fun readOnlyWhitelistMatchesSafetyRule() {
        val allowed = repoFile("claude/rules/device-safety.md").readText()
            .substringBefore("## Ask the user first").lowercase()
        for (c in Codes.READ_ONLY) assertTrue(code(c) in allowed, "${code(c)} missing from device-safety.md")
    }
}

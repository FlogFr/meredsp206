package dsp206

import java.io.File

/** Repository root, passed by Gradle (core/build.gradle.kts). */
fun repoRoot(): File = File(System.getProperty("repo.root") ?: error("repo.root is not set: run the tests with Gradle"))

fun repoFile(path: String): File = repoRoot().resolve(path)

/** One frame of a capture under tools/re/captures/: direction, bytes and, for USB, the raw report. */
data class Captured(val dir: String, val bytes: ByteArray, val report: ByteArray?)

private val FRAME = Regex(""""dir":\s*"(tx|rx)",\s*"hex":\s*"([0-9a-f]+)"(?:,\s*"report":\s*"([0-9a-f]+)")?""")

fun unhex(s: String): ByteArray = ByteArray(s.length / 2) { s.substring(2 * it, 2 * it + 2).toInt(16).toByte() }

/** Frames of a capture file, in order, without a JSON dependency. */
fun capture(name: String): List<Captured> =
    FRAME.findAll(repoFile("tools/re/captures/$name.json").readText()).map {
        Captured(it.groupValues[1], unhex(it.groupValues[2]), it.groupValues[3].takeIf { r -> r.isNotEmpty() }?.let(::unhex))
    }.toList()

/** Deterministic pseudo-random bytes (xorshift), as in the Rust tests. */
fun noise(seed: Int, n: Int): ByteArray {
    var x = if (seed == 0) 1 else seed
    return ByteArray(n) {
        x = x xor (x shl 13)
        x = x xor (x ushr 17)
        x = x xor (x shl 5)
        x.toByte()
    }
}

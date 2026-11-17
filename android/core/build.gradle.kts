// Pure Kotlin port of dsp206-proto and dsp206-link: no Android API, so it is
// built and tested on any JVM (`./gradlew :core:test`).
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
        allWarningsAsErrors.set(true)
    }
}

dependencies {
    testImplementation(kotlin("test-junit"))
    testImplementation(libs.junit)
}

tasks.test {
    // Tests read the real-device captures and the protocol docs of the repository.
    systemProperty("repo.root", rootProject.projectDir.parentFile.absolutePath)
    inputs.dir(rootProject.projectDir.parentFile.resolve("tools/re/captures"))
    inputs.file(rootProject.projectDir.parentFile.resolve("claude/docs/protocol.md"))
    testLogging {
        events("failed")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}

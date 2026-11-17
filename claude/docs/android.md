# Android app

Status (2026-11-17): **first version, not yet tried on a phone or on the real
unit.** It builds (debug APK, release APK and App Bundle), its JVM tests and
Robolectric UI tests pass, and Android lint is clean. Everything was checked
against the simulated unit only.

The app controls a DSP 206 from an Android phone over Wi-Fi (the unit's
Ethernet port on the same LAN) or over USB with an OTG adapter. The user's
test phone is a Motorola Moto G86 (Android 15 or later). The goal is to publish
it on Google Play.

## 1. Layout

```
android/                  Gradle project, independent of the Cargo workspace
  settings.gradle.kts     modules :core and :app
  gradle/libs.versions.toml
  core/                   pure Kotlin/JVM, no Android API
    src/main/kotlin/dsp206/proto/   port of crates/dsp206-proto
    src/main/kotlin/dsp206/link/    port of crates/dsp206-link (worker, TCP, HID, discovery, MockDevice)
    src/main/kotlin/dsp206/editor/  port of the GTK app's model, format, response and event handling
    src/test/kotlin/                JVM tests, including the real-device captures
  app/                    Android application (Jetpack Compose, Material 3)
    src/main/kotlin/io/github/flogfr/meredsp206/
      DspViewModel.kt     one Link at a time, Session, Compose state
      device/             LocalNetwork (Wi-Fi binding), UsbConnector (USB host)
      ui/                 screens: Device, Mixer, Channel, Presets, Log
    src/test/kotlin/      Robolectric UI tests (demo mode), screenshots
```

### 1.1 Why a Kotlin port and not the Rust crates

The protocol and link logic is ported to Kotlin instead of calling the Rust
crates through JNI. A plain Kotlin project opens and builds in Android Studio
without the NDK, Rust cross-compilers or `unsafe` FFI code (forbidden in the
workspace), and the app stays small (1.5 MB release APK). The cost is two
implementations of the protocol. These keep them in step:

- `core` mirrors the Rust modules and their tests one to one (same examples,
  same expected bytes).
- `CapturesTest.kt` replays the same `tools/re/captures/*.json` frames as
  `crates/dsp206-proto/tests/captures.rs`.
- `DocsSyncTest.kt` checks the command codes against `protocol.md`,
  `device-safety.md` and the Rust `ALL_CODES` / `READ_ONLY_CODES`.
- `tools/tests/test_android.py` checks the same codes, the USB IDs of the
  device filter and the French translation from `make check`, without a JDK.

**Any protocol change goes into both implementations in the same commit.**

### 1.2 Differences from the Rust code

- `Program` keeps the 928-byte block itself and reads every field at its
  documented offset (memory-map.md §2), instead of a struct. Conversion to and
  from bytes is lossless by construction; `copy()` makes snapshots for undo.
- `Link` takes any `Transport`: `TcpTransport` (socket from the Wi-Fi network's
  socket factory), `HidTransport` over a `HidPort` (64-byte reports, input cut
  by frame length, protocol.md §1.2) and `MockTransport` (in-process simulated
  unit, used by the demo mode and the tests).
- `Session` (core) is the GTK app's `App::poll`: it turns link events into
  model updates and typed notices; the app only formats them.
- `linkRequests()` ports the GTK link buttons (`set_link`).

## 2. Build and test

Requirements: JDK 17 or newer, the Android SDK with platform 37 and
build-tools 36 (Android Studio installs them). Versions: Gradle 9.9.1 (wrapper),
Android Gradle plugin 9.5.1 (built-in Kotlin), Kotlin 2.4.21, Compose BOM
2026.09.00.

| Command (from the repository root) | What it does | Needs the SDK |
|---|---|---|
| `make android-core` | `core` JVM tests | no |
| `make android-check` | `core` tests, Robolectric UI tests, Android lint | yes |
| `make android-apk` | debug APK in `android/app/build/outputs/apk/debug/` | yes |

The UI tests (`app/src/test/.../AppTest.kt`) start the demo, visit every
screen, mute and unmute a channel, and write screenshots to
`android/app/build/screenshots/`.

`.github/workflows/android.yml` runs the same checks on GitHub and keeps the
debug APK as a downloadable artifact.

SDK levels: `compileSdk` 37 (Compose 1.12 and core 1.19 require it), `targetSdk`
36, `minSdk` 26. `targetSdk` 37 (Android 17) turns on the local network
permission; raise it only after testing that on a phone. Google Play's yearly
target-API deadline should accept 36 until the end of August 2027 [guess]:
check the Play Console before each release.

## 3. Behaviour on the phone

- **Network.** The app follows the Wi-Fi (or Ethernet) network with a
  `NetworkCallback` and creates the TCP socket from that network's socket
  factory, so it works on a Wi-Fi without internet access even when mobile
  data is on. Discovery sends the announce probe (protocol.md §1.1) to
  `255.255.255.255` and to the subnet broadcast address, from a socket bound to
  the same network. The rules of connection.md §2 apply unchanged: one
  connection, one request in flight, 800 ms timeout, coalescing, a 60 s
  countdown after a network session ends, and a warning when connecting earlier.
- **USB.** `UsbConnector` finds the unit by VID/PID, asks the user for
  permission, claims the HID interface and exchanges 64-byte reports on the
  interrupt endpoints (SET_REPORT on the control endpoint if there is no OUT
  endpoint). The manifest offers to open the app when the unit is plugged in.
  Not tried on hardware yet.
- **Meters** are polled only while the Mixer or Channel screen is visible and
  the app is in the foreground.
- **Safety.** Loading a preset, storing one and sending the whole program ask
  for confirmation. Edits are sent at once, as in the GTK app.
- **Offline.** Without a connection, edits change the local program, which can
  be saved to a `.dsp206` file (same format as the GTK app) and sent later.
- Languages: English (default) and French (`values-fr`).

## 4. Publishing on Google Play (checklist)

1. Create an upload key once and keep it outside the repository:
   `keytool -genkeypair -v -keystore upload.jks -alias upload -keyalg RSA -keysize 4096 -validity 10000`.
2. Create `android/keystore.properties` (ignored by git) with `storeFile`,
   `storePassword`, `keyAlias`, `keyPassword`. `storeFile` is relative to
   `android/`.
3. Raise `versionCode` (and `versionName`) in `app/build.gradle.kts` for every
   upload.
4. `cd android && ./gradlew :app:bundleRelease` →
   `app/build/outputs/bundle/release/app-release.aab`.
5. In Play Console: create the app, enable Play App Signing, upload the bundle
   to internal testing first, fill in the data safety form (the app collects
   no data and has no network service of its own), a privacy policy URL,
   screenshots (the UI tests produce some) and the content rating.
6. The application ID `io.github.flogfr.meredsp206` cannot change after the
   first upload.
7. Name and listing: say "for the t.racks DSP 206" but do not use the t.racks
   or Thomann names or logos as the app's name or icon, and state that the app
   is not affiliated with them.

## 5. Open points

- Not yet run on a real phone, nor against the real unit over Wi-Fi or USB.
- Over the network the unit stops answering after 1–2 s (connection.md). The
  app shows it ("not answering") and keeps sending; it cannot fix it. USB OTG is
  the workaround to try.
- Lock/PTS, test tone, device ID/IP setup and HPF/LPF bypass are not in the UI
  (as in the GTK app).

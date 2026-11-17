# meredsp206

A native Linux editor for the **t.racks DSP 206** loudspeaker processor (2 in /
6 out), controlled over USB or the local network. It aims to cover every feature
of the Windows-only *the t.racks DSP 206 Processor Editor V1.05*, with a
cleaner, faster interface. An **Android app** controls the same unit from a
phone, over Wi-Fi or USB OTG (see [Android app](#android-app)).

> **Project status: early.** The device protocol has been reverse engineered and
> documented ([claude/docs/protocol.md](claude/docs/protocol.md)). The protocol
> library, the link (USB and network), the `dsp206` command-line tool and the
> `meredsp206` GUI are written and tested against a simulated unit, but not yet
> fully checked on real hardware. The sections below marked *(planned)*
> describe the target.

## Features (planned)

- Connect over USB, find DSP 206 units on the LAN automatically, or connect by IP.
- Mixer view: all 8 channels at a glance with level meters, gain, mute, polarity
  and names.
- Per input: gain, mute, polarity, delay, noise gate, 31-band graphic EQ,
  8-band parametric EQ, high-pass and low-pass filters.
- Per output: input routing matrix with mix levels, gain, mute, polarity, delay,
  9-band parametric EQ, high-pass/low-pass (Butterworth, Bessel, Linkwitz-Riley up
  to 48 dB/oct), RMS compressor and peak limiter.
- Channel linking and copying, channel names.
- Presets on the device: list, recall, store with a name. Save and load programs
  as files.
- Test tone generator (pink noise, white noise, sine), device lock.
- Offline mode to prepare a program without the device.

The target look is in [`design/`](design/):

![Mixer view](design/mixer-view.png)

## Requirements

- Debian 13 (trixie) or newer, or another recent Linux distribution with GTK 4.
- A t.racks DSP 206, plugged into a USB port or reachable on your network
  (factory IP `192.168.1.101`, TCP port `9761`).

## USB access

Over USB the unit appears as a HID device named *Musicrown Dsp Process*
(`0168:0821`), for example `/dev/hidraw5`. Linux gives that node to root
only. Install the udev rule once so your user can open it:

```bash
sudo install -m 644 data/udev/70-meredsp206.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw
```

`dsp206 discover` lists the units it finds on USB and on the network.

USB is the recommended link: on our unit every request is answered within
about 20 ms, while the network link loses replies after a second or two (see
[connection behaviour](claude/docs/connection.md)).

## Build from source

Install the build dependencies:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev cargo rustc
```

Build:

```bash
cargo build --release
```

The binaries are written to `target/release/`: `meredsp206` (GUI) and `dsp206`
(command-line tool).

## Install (planned)

```bash
sudo make install PREFIX=/usr/local
```

This installs both binaries, a desktop entry and an icon. Remove them with
`sudo make uninstall PREFIX=/usr/local`.

## Usage

1. Plug the DSP 206 into a USB port (see *USB access*), or connect it to the
   same network as your computer.
2. Start the GUI with `cargo run -p dsp206-gtk` (add `-- --usb` to connect over
   USB at once, or `-- --connect IP`). On the *Device* page, use *Connect* on
   the USB row, or enter the IP address and use *Connect* below it.
3. The current program is read from the device. Every change you make is sent
   immediately.

The command-line tool does the same from a terminal:

```bash
cargo run -p dsp206-cli -- --usb info          # identity, presets, current program
cargo run -p dsp206-cli -- --usb meters 10     # ten level-meter readings
cargo run -p dsp206-cli -- --host 192.168.1.101 info
```

`--hidraw /dev/hidrawN` picks a USB node when several are plugged in. Run
`cargo run -p dsp206-cli -- help` for every command.

Good to know: the DSP accepts only one network connection at a time. Close the
Windows editor or any other client first. After a network disconnection, wait
about one minute before reconnecting.

## Android app

The app in [`android/`](android/) controls the unit from an Android phone
(Android 8.0 or newer): over Wi-Fi when the unit's Ethernet port is on the same
network as the phone, or over USB with a USB-C OTG adapter. It has a mixer view
with meters, a channel editor (gain, delay, parametric and graphic EQ,
crossover, gate, compressor, limiter, routing, links), presets, program files,
a protocol log and a demo mode with a simulated unit. English and French.

> **Status: first version.** Tested against the simulated unit only; not yet
> on a phone or on the real unit. Over the network, our unit stops answering a
> second or two after connecting ([connection behaviour](claude/docs/connection.md));
> try USB OTG if that happens.

### Try it on a phone

1. On the phone: *Settings → About phone*, tap *Build number* seven times, then
   enable *Developer options → USB debugging*.
2. Install [Android Studio](https://developer.android.com/studio), open the
   `android/` folder, plug the phone in over USB, accept the debugging prompt
   on the phone and press *Run*.

Or from a terminal, with the Android SDK installed (`ANDROID_HOME` set):

```bash
cd android && ./gradlew installDebug
```

Or install the APK file directly: download `app-debug.apk` (from the
*Android* workflow run on GitHub, *Artifacts*), open it on the phone and allow
installing apps from that source.

In the app: *Device* page → *Start the demo* to look around without a unit, or
*Search the network* / type the unit's address (factory `192.168.1.101`) and
*Connect*. The phone and the unit must be on the same subnet.

### Build and check

```bash
make android-core     # protocol tests on the JVM (JDK 17+ only)
make android-check    # + UI tests on Robolectric and Android lint (needs the SDK)
make android-apk      # android/app/build/outputs/apk/debug/app-debug.apk
```

Publishing on Google Play (signing, App Bundle, listing) is described in
[claude/docs/android.md](claude/docs/android.md#4-publishing-on-google-play-checklist).

## Development

Requirements: Rust 1.84 or newer (`cargo`, `rustfmt`, `clippy`), Python 3, `make`.

```bash
make check
```

This runs the formatter check, clippy with warnings as errors, the Rust tests
(including replays of frames captured on a real DSP 206) and the Python tests
for the tools and the documentation. Other targets: `make test`, `make lint`,
`make fmt`, `make design`.

## Documentation

- [Product overview and feature inventory](claude/docs/overview.md)
- [Protocol (USB and network)](claude/docs/protocol.md)
- [Program memory map and value encodings](claude/docs/memory-map.md)
- [Connection behaviour](claude/docs/connection.md)
- [Planned architecture](claude/docs/architecture.md)
- [Android app](claude/docs/android.md)

## Legal

This project is not affiliated with Thomann or the manufacturer of the DSP 206.
The protocol was studied for interoperability. No vendor code or assets are
included in this repository.

//! Typed requests and replies (protocol.md §4).
//!
//! Setter payloads follow protocol.md §4.3 exactly. Multi-byte values are
//! little-endian u16. Channel numbers are global: 0 = InA, 1 = InB,
//! 2..=7 = Out1..Out6.

use crate::frame::Frame;
use crate::program::{Compressor, CutFilter, Gate, Limiter, PeqBand, NAME_LEN, PROGRAM_NAME_LEN};
use crate::units::meter_dbu;

/// A DSP 206 channel, 0..=7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Channel(u8);

impl Channel {
    pub const IN_A: Channel = Channel(0);
    pub const IN_B: Channel = Channel(1);
    pub const COUNT: u8 = 8;

    pub fn new(index: u8) -> Option<Channel> {
        (index < Self::COUNT).then_some(Channel(index))
    }

    /// Output `n`, 1-based (Out1..Out6).
    pub fn output(n: u8) -> Option<Channel> {
        (1..=6).contains(&n).then_some(Channel(n + 1))
    }

    pub fn index(self) -> u8 {
        self.0
    }

    pub fn is_input(self) -> bool {
        self.0 < 2
    }

    pub fn label(self) -> String {
        if self.is_input() {
            format!("In{}", (b'A' + self.0) as char)
        } else {
            format!("Out{}", self.0 - 1)
        }
    }

    pub fn all() -> impl Iterator<Item = Channel> {
        (0..Self::COUNT).map(Channel)
    }
}

/// Requests sent by the PC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    // Session and information (protocol.md §4.1)
    Connect,
    Disconnect,
    Identify,
    GetCurrentPreset,
    GetLockState,
    GetMeters,
    /// `0x12`, sent after a bulk read. Effect not confirmed.
    EndOfTransfer,
    // Presets and bulk transfer (§4.2)
    GetPresetSlots,
    GetPresetName(u8),
    ReadProgramPage(u8),
    /// `0x26`: name of the current program, sent before a store.
    SetPresetName([u8; PROGRAM_NAME_LEN]),
    /// `0x21 [n]`: store the current program into user preset `n` (1 = U01).
    StorePreset(u8),
    /// `0x20 [n]`: load preset `n` (0 = F00, 1 = U01) as the current program.
    RecallPreset(u8),
    // Parameter setters (§4.3)
    SetGain {
        ch: Channel,
        raw: u16,
    },
    SetMute {
        ch: Channel,
        on: bool,
    },
    SetPolarity {
        ch: Channel,
        inverted: bool,
    },
    SetDelay {
        ch: Channel,
        raw: u16,
    },
    SetDelayUnit(u8),
    SetPeqBand {
        ch: Channel,
        band: u8,
        value: PeqBand,
        bypass: bool,
    },
    SetPeqBypass {
        ch: Channel,
        on: bool,
    },
    SetGeqBand {
        ch: Channel,
        band: u8,
        gain: u16,
    },
    SetGeqBypass {
        ch: Channel,
        on: bool,
    },
    SetHighPass {
        ch: Channel,
        filter: CutFilter,
    },
    SetLowPass {
        ch: Channel,
        filter: CutFilter,
    },
    SetGate {
        ch: Channel,
        gate: Gate,
    },
    SetCompressor {
        ch: Channel,
        comp: Compressor,
    },
    SetLimiter {
        ch: Channel,
        limiter: Limiter,
    },
    SetRouting {
        ch: Channel,
        mask: u8,
    },
    SetMixGain {
        ch: Channel,
        input: u8,
        raw: u16,
    },
    SetLink {
        ch: Channel,
        mask: u8,
    },
    CopyChannel {
        src: Channel,
        dst: Channel,
    },
    SetChannelName {
        ch: Channel,
        name: [u8; NAME_LEN],
    },
    /// Test-tone generator: `source` is a [`ToneSource`] value, `freq` the sine
    /// frequency as an index into `units::GEQ_BANDS_HZ` (0 = 20 Hz).
    TestTone {
        source: u8,
        freq: u8,
    },
}

/// Test-tone sources (`TfmTest` radio button tags) [code].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneSource {
    AnalogInput = 0,
    PinkNoise = 1,
    WhiteNoise = 2,
    Sine = 3,
}

/// Reply to setters and preset operations (protocol.md §3.3).
pub const ACK: u8 = 0x01;
/// Reply to a frame the device could not take, probably a damaged one
/// (protocol.md §3.3).
pub const NAK: u8 = 0x02;

/// Command codes that only read device state (claude/rules/device-safety.md).
pub const READ_ONLY_CODES: [u8; 9] = [0x10, 0x11, 0x13, 0x14, 0x22, 0x27, 0x29, 0x2C, 0x40];

/// Every command code a [`Request`] can produce, sorted.
pub const ALL_CODES: [u8; 33] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x20, 0x21, 0x22, 0x26, 0x27, 0x29, 0x2A, 0x2C, 0x30, 0x31,
    0x32, 0x33, 0x34, 0x35, 0x36, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x48,
    0x49,
];

fn words(out: &mut Vec<u8>, ws: &[u16]) {
    ws.iter()
        .for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
}

impl Request {
    /// Command byte.
    pub fn code(&self) -> u8 {
        use Request::*;
        match self {
            Connect => 0x10,
            Disconnect => 0x11,
            EndOfTransfer => 0x12,
            Identify => 0x13,
            GetCurrentPreset => 0x14,
            SetDelayUnit(_) => 0x15,
            RecallPreset(_) => 0x20,
            StorePreset(_) => 0x21,
            GetPresetSlots => 0x22,
            SetPresetName(_) => 0x26,
            ReadProgramPage(_) => 0x27,
            GetPresetName(_) => 0x29,
            CopyChannel { .. } => 0x2A,
            GetLockState => 0x2C,
            SetCompressor { .. } => 0x30,
            SetLowPass { .. } => 0x31,
            SetHighPass { .. } => 0x32,
            SetPeqBand { .. } => 0x33,
            SetGain { .. } => 0x34,
            SetMute { .. } => 0x35,
            SetPolarity { .. } => 0x36,
            SetDelay { .. } => 0x38,
            TestTone { .. } => 0x39,
            SetRouting { .. } => 0x3A,
            SetLink { .. } => 0x3B,
            SetPeqBypass { .. } => 0x3C,
            SetChannelName { .. } => 0x3D,
            SetGate { .. } => 0x3E,
            SetLimiter { .. } => 0x3F,
            GetMeters => 0x40,
            SetMixGain { .. } => 0x41,
            SetGeqBand { .. } => 0x48,
            SetGeqBypass { .. } => 0x49,
        }
    }

    /// Payload bytes (everything between `cmd` and `10 03`).
    pub fn payload(&self) -> Vec<u8> {
        use Request::*;
        let mut d = Vec::new();
        match self {
            Connect | Disconnect | EndOfTransfer | Identify | GetCurrentPreset | GetLockState
            | GetMeters | GetPresetSlots => {}
            GetPresetName(n) | ReadProgramPage(n) | StorePreset(n) | RecallPreset(n)
            | SetDelayUnit(n) => d.push(*n),
            SetPresetName(name) => d.extend_from_slice(name),
            SetGain { ch, raw } | SetDelay { ch, raw } => {
                d.push(ch.0);
                words(&mut d, &[*raw]);
            }
            SetMute { ch, on } | SetPeqBypass { ch, on } | SetGeqBypass { ch, on } => {
                d.extend_from_slice(&[ch.0, u8::from(*on)]);
            }
            SetPolarity { ch, inverted } => d.extend_from_slice(&[ch.0, u8::from(*inverted)]),
            SetPeqBand {
                ch,
                band,
                value,
                bypass,
            } => {
                d.extend_from_slice(&[ch.0, *band]);
                words(&mut d, &[value.gain, value.freq]);
                d.extend_from_slice(&[value.q, value.kind, u8::from(*bypass)]);
            }
            SetGeqBand { ch, band, gain } => {
                d.extend_from_slice(&[ch.0, *band]);
                words(&mut d, &[*gain]);
            }
            SetHighPass { ch, filter } | SetLowPass { ch, filter } => {
                d.push(ch.0);
                words(&mut d, &[filter.freq]);
                d.push(filter.slope);
            }
            SetGate { ch, gate: g } => {
                d.push(ch.0);
                words(&mut d, &[g.attack, g.hold, g.release, g.threshold]);
            }
            SetCompressor { ch, comp: c } => {
                d.push(ch.0);
                words(&mut d, &[c.ratio, c.attack, c.release, c.knee, c.threshold]);
            }
            SetLimiter { ch, limiter: l } => {
                d.push(ch.0);
                words(&mut d, &[l.attack, l.release, l.knee, l.threshold]);
            }
            SetRouting { ch, mask } | SetLink { ch, mask } => d.extend_from_slice(&[ch.0, *mask]),
            SetMixGain { ch, input, raw } => {
                d.extend_from_slice(&[ch.0, *input]);
                words(&mut d, &[*raw]);
            }
            CopyChannel { src, dst } => d.extend_from_slice(&[src.0, dst.0]),
            SetChannelName { ch, name } => {
                d.push(ch.0);
                d.extend_from_slice(name);
            }
            TestTone { source, freq } => d.extend_from_slice(&[*source, *freq]),
        }
        d
    }

    /// Decode a request from its command byte and payload (inverse of
    /// [`Request::payload`]). Used by the simulated device.
    pub fn parse(cmd: u8, d: &[u8]) -> Option<Request> {
        use Request::*;
        let w =
            |i: usize| -> Option<u16> { Some(u16::from_le_bytes([*d.get(i)?, *d.get(i + 1)?])) };
        let b = |i: usize| -> Option<u8> { d.get(i).copied() };
        let ch = |i: usize| -> Option<Channel> { Channel::new(*d.get(i)?) };
        let exact = |n: usize| (d.len() == n).then_some(());
        Some(match cmd {
            0x10 => exact(0).map(|_| Connect)?,
            0x11 => exact(0).map(|_| Disconnect)?,
            0x12 => exact(0).map(|_| EndOfTransfer)?,
            0x13 => exact(0).map(|_| Identify)?,
            0x14 => exact(0).map(|_| GetCurrentPreset)?,
            0x15 => SetDelayUnit(b(0)?),
            0x20 => RecallPreset(b(0)?),
            0x21 => StorePreset(b(0)?),
            0x22 => exact(0).map(|_| GetPresetSlots)?,
            0x26 => SetPresetName(d.try_into().ok()?),
            0x27 => ReadProgramPage(b(0)?),
            0x29 => GetPresetName(b(0)?),
            0x2A => CopyChannel {
                src: ch(0)?,
                dst: ch(1)?,
            },
            0x2C => exact(0).map(|_| GetLockState)?,
            0x30 => SetCompressor {
                ch: ch(0)?,
                comp: Compressor {
                    ratio: w(1)?,
                    attack: w(3)?,
                    release: w(5)?,
                    knee: w(7)?,
                    threshold: w(9)?,
                },
            },
            0x31 | 0x32 => {
                let filter = CutFilter {
                    freq: w(1)?,
                    slope: b(3)?,
                };
                if cmd == 0x31 {
                    SetLowPass { ch: ch(0)?, filter }
                } else {
                    SetHighPass { ch: ch(0)?, filter }
                }
            }
            0x33 => SetPeqBand {
                ch: ch(0)?,
                band: b(1)?,
                value: PeqBand {
                    gain: w(2)?,
                    freq: w(4)?,
                    q: b(6)?,
                    kind: b(7)?,
                },
                bypass: b(8)? != 0,
            },
            0x34 => SetGain {
                ch: ch(0)?,
                raw: w(1)?,
            },
            0x35 => SetMute {
                ch: ch(0)?,
                on: b(1)? != 0,
            },
            0x36 => SetPolarity {
                ch: ch(0)?,
                inverted: b(1)? != 0,
            },
            0x38 => SetDelay {
                ch: ch(0)?,
                raw: w(1)?,
            },
            0x39 => TestTone {
                source: b(0)?,
                freq: b(1)?,
            },
            0x3A => SetRouting {
                ch: ch(0)?,
                mask: b(1)?,
            },
            0x3B => SetLink {
                ch: ch(0)?,
                mask: b(1)?,
            },
            0x3C => SetPeqBypass {
                ch: ch(0)?,
                on: b(1)? != 0,
            },
            0x3D => SetChannelName {
                ch: ch(0)?,
                name: d.get(1..1 + NAME_LEN)?.try_into().ok()?,
            },
            0x3E => SetGate {
                ch: ch(0)?,
                gate: Gate {
                    attack: w(1)?,
                    hold: w(3)?,
                    release: w(5)?,
                    threshold: w(7)?,
                },
            },
            0x3F => SetLimiter {
                ch: ch(0)?,
                limiter: Limiter {
                    attack: w(1)?,
                    release: w(3)?,
                    knee: w(5)?,
                    threshold: w(7)?,
                },
            },
            0x40 => exact(0).map(|_| GetMeters)?,
            0x41 => SetMixGain {
                ch: ch(0)?,
                input: b(1)?,
                raw: w(2)?,
            },
            0x48 => SetGeqBand {
                ch: ch(0)?,
                band: b(1)?,
                gain: w(2)?,
            },
            0x49 => SetGeqBypass {
                ch: ch(0)?,
                on: b(1)? != 0,
            },
            _ => return None,
        })
    }

    /// The frame for device `dev`.
    pub fn frame(&self, dev: u8) -> Frame {
        Frame::request(dev, self.code(), self.payload())
    }

    /// True when the command cannot change sound or device state.
    pub fn is_read_only(&self) -> bool {
        READ_ONLY_CODES.contains(&self.code())
    }

    /// Command byte of the reply that completes this request, when known.
    /// Setters and preset operations are acknowledged with [`ACK`]
    /// (seen on the unit for `0x35`, protocol.md §3.3).
    pub fn reply_code(&self) -> Option<u8> {
        use Request::*;
        match self {
            ReadProgramPage(_) => Some(0x24),
            Connect | Identify | GetCurrentPreset | GetLockState | GetMeters | GetPresetSlots
            | GetPresetName(_) => Some(self.code()),
            Disconnect | EndOfTransfer => None,
            _ => Some(ACK),
        }
    }

    /// True for requests that change the device (answered by [`ACK`]).
    pub fn is_write(&self) -> bool {
        self.reply_code() == Some(ACK)
    }

    /// Queued setters with the same key replace each other (protocol.md §3.2).
    /// Reads are never coalesced.
    pub fn coalesce_key(&self) -> Option<(u8, u8, u8)> {
        use Request::*;
        match self {
            SetPeqBand { ch, band, .. } | SetGeqBand { ch, band, .. } => {
                Some((self.code(), ch.0, *band))
            }
            SetMixGain { ch, input, .. } => Some((self.code(), ch.0, *input)),
            SetGain { ch, .. }
            | SetMute { ch, .. }
            | SetPolarity { ch, .. }
            | SetDelay { ch, .. }
            | SetPeqBypass { ch, .. }
            | SetGeqBypass { ch, .. }
            | SetHighPass { ch, .. }
            | SetLowPass { ch, .. }
            | SetGate { ch, .. }
            | SetCompressor { ch, .. }
            | SetLimiter { ch, .. }
            | SetRouting { ch, .. }
            | SetLink { ch, .. }
            | SetChannelName { ch, .. } => Some((self.code(), ch.0, 0)),
            SetDelayUnit(_) | TestTone { .. } => Some((self.code(), 0, 0)),
            _ => None,
        }
    }
}

/// Decoded `0x40` meter reply (protocol.md §4.6).
#[derive(Clone, Debug, PartialEq)]
pub struct Meters {
    /// InA, InB in dBu.
    pub inputs: [f64; 2],
    /// Out1..Out6 in dBu.
    pub outputs: [f64; 6],
    /// Bytes 30..32, compressor/limiter activity (meaning not confirmed).
    pub status: u16,
}

impl Meters {
    pub const MIN_LEN: usize = 32;

    pub fn parse(data: &[u8]) -> Option<Meters> {
        if data.len() < Self::MIN_LEN {
            return None;
        }
        let group = |i: usize| meter_dbu([data[3 * i], data[3 * i + 1], data[3 * i + 2]]);
        Some(Meters {
            inputs: [group(0), group(1)],
            // Groups 2 and 3 are unused on the DSP 206.
            outputs: [group(4), group(5), group(6), group(7), group(8), group(9)],
            status: u16::from_le_bytes([data[30], data[31]]),
        })
    }
}

/// Replies sent by the device.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Ack,
    /// `0x02`, seen once when the device was busy (meaning not confirmed).
    Nak,
    Connected {
        status: u8,
    },
    Identity(String),
    CurrentPreset(u8),
    PresetSlots(Vec<u8>),
    PresetName {
        index: u8,
        name: String,
    },
    ProgramPage {
        page: u8,
        data: Vec<u8>,
    },
    LockState(Vec<u8>),
    Meters(Meters),
    /// Anything not decoded yet, kept whole.
    Other(Frame),
}

impl Reply {
    pub fn parse(frame: &Frame) -> Reply {
        let d = &frame.data;
        match (frame.cmd, d.len()) {
            (0x01, _) => Reply::Ack,
            (0x02, _) => Reply::Nak,
            (0x10, 1..) => Reply::Connected { status: d[0] },
            (0x13, _) => Reply::Identity(String::from_utf8_lossy(d).trim_end().to_string()),
            (0x14, 1..) => Reply::CurrentPreset(d[0]),
            (0x22, _) => Reply::PresetSlots(d.clone()),
            (0x29, 1..) => Reply::PresetName {
                index: d[0],
                name: crate::program::name_text(&d[1..]),
            },
            (0x24, 1..) => Reply::ProgramPage {
                page: d[0],
                data: d[1..].to_vec(),
            },
            (0x2C, _) => Reply::LockState(d.clone()),
            (0x40, _) => match Meters::parse(d) {
                Some(m) => Reply::Meters(m),
                None => Reply::Other(frame.clone()),
            },
            _ => Reply::Other(frame.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units;

    fn ch(i: u8) -> Channel {
        Channel::new(i).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// One example of every request with its expected payload.
    fn examples() -> Vec<(Request, &'static str)> {
        let band = PeqBand {
            gain: 120,
            freq: 170,
            q: 35,
            kind: 0,
        };
        vec![
            (Request::Connect, ""),
            (Request::Disconnect, ""),
            (Request::EndOfTransfer, ""),
            (Request::Identify, ""),
            (Request::GetCurrentPreset, ""),
            (Request::GetLockState, ""),
            (Request::GetMeters, ""),
            (Request::GetPresetSlots, ""),
            (Request::GetPresetName(20), "14"),
            (Request::ReadProgramPage(18), "12"),
            (
                Request::SetPresetName(*b"Main PA 3-way\0"),
                "4d 61 69 6e 20 50 41 20 33 2d 77 61 79 00",
            ),
            (Request::StorePreset(3), "03"),
            (Request::RecallPreset(3), "03"),
            (
                Request::SetGain {
                    ch: ch(2),
                    raw: 280,
                },
                "02 18 01",
            ),
            (
                Request::SetMute {
                    ch: ch(7),
                    on: true,
                },
                "07 01",
            ),
            (
                Request::SetPolarity {
                    ch: ch(0),
                    inverted: true,
                },
                "00 01",
            ),
            (
                Request::SetDelay {
                    ch: ch(3),
                    raw: 65280,
                },
                "03 00 ff",
            ),
            (Request::SetDelayUnit(2), "02"),
            (
                Request::SetPeqBand {
                    ch: ch(1),
                    band: 7,
                    value: band,
                    bypass: true,
                },
                "01 07 78 00 aa 00 23 00 01",
            ),
            (
                Request::SetPeqBypass {
                    ch: ch(4),
                    on: false,
                },
                "04 00",
            ),
            (
                Request::SetGeqBand {
                    ch: ch(0),
                    band: 30,
                    gain: 240,
                },
                "00 1e f0 00",
            ),
            (
                Request::SetGeqBypass {
                    ch: ch(1),
                    on: true,
                },
                "01 01",
            ),
            (
                Request::SetHighPass {
                    ch: ch(2),
                    filter: CutFilter {
                        freq: 300,
                        slope: 9,
                    },
                },
                "02 2c 01 09",
            ),
            (
                Request::SetLowPass {
                    ch: ch(2),
                    filter: CutFilter { freq: 0, slope: 19 },
                },
                "02 00 00 13",
            ),
            (
                Request::SetGate {
                    ch: ch(0),
                    gate: Gate {
                        attack: 0,
                        hold: 99,
                        release: 99,
                        threshold: 0,
                    },
                },
                "00 00 00 63 00 63 00 00 00",
            ),
            (
                Request::SetCompressor {
                    ch: ch(2),
                    comp: Compressor {
                        ratio: 0,
                        attack: 49,
                        release: 499,
                        knee: 0,
                        threshold: 220,
                    },
                },
                "02 00 00 31 00 f3 01 00 00 dc 00",
            ),
            (
                Request::SetLimiter {
                    ch: ch(2),
                    limiter: Limiter {
                        attack: 49,
                        release: 499,
                        knee: 0,
                        threshold: 220,
                    },
                },
                "02 31 00 f3 01 00 00 dc 00",
            ),
            (Request::SetRouting { ch: ch(2), mask: 3 }, "02 03"),
            (
                Request::SetMixGain {
                    ch: ch(2),
                    input: 1,
                    raw: 280,
                },
                "02 01 18 01",
            ),
            (
                Request::SetLink {
                    ch: ch(2),
                    mask: 0x3f,
                },
                "02 3f",
            ),
            (
                Request::CopyChannel {
                    src: ch(2),
                    dst: ch(3),
                },
                "02 03",
            ),
            (
                Request::SetChannelName {
                    ch: ch(2),
                    name: *b"Sub L\0\0\0",
                },
                "02 53 75 62 20 4c 00 00 00",
            ),
            (
                Request::TestTone {
                    source: ToneSource::Sine as u8,
                    freq: 17,
                },
                "03 11",
            ),
        ]
    }

    #[test]
    fn payloads_match_protocol_doc() {
        for (req, expected) in examples() {
            assert_eq!(hex(&req.payload()), expected, "{req:?}");
        }
    }

    #[test]
    fn parse_inverts_payload() {
        for (req, _) in examples() {
            assert_eq!(
                Request::parse(req.code(), &req.payload()),
                Some(req.clone()),
                "{req:?}"
            );
        }
        assert_eq!(Request::parse(0x34, &[0x02, 0x18]), None); // truncated
        assert_eq!(Request::parse(0x34, &[0x08, 0x18, 0x01]), None); // bad channel
        assert_eq!(Request::parse(0x10, &[0x00]), None); // unexpected payload
        assert_eq!(Request::parse(0x77, &[]), None);
    }

    #[test]
    fn every_code_is_unique_and_has_an_example() {
        let mut codes: Vec<u8> = examples().iter().map(|(r, _)| r.code()).collect();
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), n, "two requests share a command code");
        assert_eq!(
            codes, ALL_CODES,
            "ALL_CODES is out of date, or a request lacks an example"
        );
    }

    #[test]
    fn every_request_fits_in_one_usb_report() {
        // The editor sends one frame per 64-byte HID report (protocol.md §1.2).
        for (req, _) in examples() {
            let bytes = req.frame(1).encode().unwrap();
            assert!(crate::hid::output_report(&bytes).is_ok(), "{req:?}");
        }
    }

    #[test]
    fn frames_are_well_formed() {
        for (req, _) in examples() {
            let bytes = req.frame(1).encode().unwrap();
            let back = Frame::decode(&bytes).unwrap();
            assert_eq!((back.src, back.dst, back.cmd), (0, 1, req.code()));
            assert_eq!(back.data, req.payload());
        }
        // A real capture: connect to device 1.
        assert_eq!(
            hex(&Request::Connect.frame(1).encode().unwrap()),
            "10 02 00 01 01 10 10 03 11"
        );
    }

    #[test]
    fn preset_codes_follow_the_original_editor() {
        // Load is `0x20` (0x426f8c), store is `0x21` sent after the `0x26`
        // ACK (0x422df1). They were swapped once, so a "store" loaded U01.
        assert_eq!(Request::RecallPreset(0).code(), 0x20);
        assert_eq!(Request::StorePreset(1).code(), 0x21);
        assert_eq!(Request::SetPresetName([b' '; 14]).code(), 0x26);
        assert_eq!(Request::parse(0x21, &[1]), Some(Request::StorePreset(1)));
        assert_eq!(Request::parse(0x20, &[0]), Some(Request::RecallPreset(0)));
        let store_u01 = Request::StorePreset(1).frame(1).encode().unwrap();
        assert_eq!(hex(&store_u01[..7]), "10 02 00 01 02 21 01");
    }

    #[test]
    fn only_documented_reads_are_read_only() {
        for (req, _) in examples() {
            assert_eq!(
                req.is_read_only(),
                READ_ONLY_CODES.contains(&req.code()),
                "{req:?}"
            );
            // A read-only request is never coalesced away.
            if req.is_read_only() {
                assert_eq!(req.coalesce_key(), None, "{req:?}");
            }
        }
        assert!(!Request::EndOfTransfer.is_read_only());
        assert!(!Request::RecallPreset(0).is_read_only());
        assert!(Request::ReadProgramPage(0).is_read_only());
    }

    #[test]
    fn coalescing_keys() {
        let a = Request::SetGain { ch: ch(2), raw: 1 };
        let b = Request::SetGain {
            ch: ch(2),
            raw: 300,
        };
        let c = Request::SetGain { ch: ch(3), raw: 1 };
        assert_eq!(a.coalesce_key(), b.coalesce_key());
        assert_ne!(a.coalesce_key(), c.coalesce_key());
        let band = |n| Request::SetGeqBand {
            ch: ch(0),
            band: n,
            gain: 120,
        };
        assert_ne!(band(1).coalesce_key(), band(2).coalesce_key());
        assert_eq!(Request::StorePreset(1).coalesce_key(), None);
    }

    #[test]
    fn reply_codes() {
        assert_eq!(Request::ReadProgramPage(0).reply_code(), Some(0x24));
        assert_eq!(Request::Identify.reply_code(), Some(0x13));
        assert_eq!(
            Request::SetGain { ch: ch(0), raw: 0 }.reply_code(),
            Some(ACK)
        );
        assert_eq!(Request::Disconnect.reply_code(), None);
        assert_eq!(Request::EndOfTransfer.reply_code(), None);
        // Every request that is not read-only, except the session frames,
        // waits for an ACK; no read-only request does.
        for (req, _) in examples() {
            let session = matches!(req, Request::Disconnect | Request::EndOfTransfer);
            assert_eq!(req.is_write(), !req.is_read_only() && !session, "{req:?}");
        }
    }

    #[test]
    fn channels() {
        assert_eq!(Channel::new(8), None);
        assert_eq!(Channel::output(1), Some(ch(2)));
        assert_eq!(Channel::output(7), None);
        let labels: Vec<String> = Channel::all().map(Channel::label).collect();
        assert_eq!(
            labels,
            ["InA", "InB", "Out1", "Out2", "Out3", "Out4", "Out5", "Out6"]
        );
        assert!(Channel::IN_B.is_input() && !ch(2).is_input());
    }

    #[test]
    fn parses_captured_replies() {
        let f = |cmd, data: &[u8]| Frame {
            src: 1,
            dst: 0,
            cmd,
            data: data.to_vec(),
        };
        assert_eq!(
            Reply::parse(&f(0x10, &[0x19])),
            Reply::Connected { status: 0x19 }
        );
        assert_eq!(
            Reply::parse(&f(0x13, b"DSP206 V0105P")),
            Reply::Identity("DSP206 V0105P".into())
        );
        assert_eq!(Reply::parse(&f(0x14, &[0])), Reply::CurrentPreset(0));
        assert_eq!(
            Reply::parse(&f(0x29, b"\x05Default Preset")),
            Reply::PresetName {
                index: 5,
                name: "Default Preset".into()
            }
        );
        assert_eq!(
            Reply::parse(&f(0x24, &[3, 0xaa, 0xbb])),
            Reply::ProgramPage {
                page: 3,
                data: vec![0xaa, 0xbb]
            }
        );
        assert_eq!(Reply::parse(&f(0x02, &[])), Reply::Nak);
        assert_eq!(Reply::parse(&f(0x01, &[])), Reply::Ack);
        assert!(matches!(Reply::parse(&f(0x77, &[1])), Reply::Other(_)));
        assert!(matches!(Reply::parse(&f(0x10, &[])), Reply::Other(_)));
    }

    #[test]
    fn parses_captured_meters() {
        // 0x40 reply payload captured on an idle unit (protocol.md §4.6).
        let data: Vec<u8> =
            "ad 36 14 ab 36 d7 00 00 00 00 00 00 ac 36 fa ab 36 d7 ad 36 14 aa 36 8e \
                             ac 36 fa aa 36 8e 00 00 00 00 00 00 00 00"
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap())
                .collect();
        let Reply::Meters(m) = Reply::parse(&Frame {
            src: 1,
            dst: 0,
            cmd: 0x40,
            data: data.clone(),
        }) else {
            panic!("not meters");
        };
        assert!(m
            .inputs
            .iter()
            .chain(&m.outputs)
            .all(|db| (-89.0..-87.0).contains(db)));
        assert_eq!(m.inputs[0], units::meter_dbu([0xad, 0x36, 0x14]));
        assert_eq!(m.status, 0);
        assert!(Meters::parse(&data[..31]).is_none());
    }
}

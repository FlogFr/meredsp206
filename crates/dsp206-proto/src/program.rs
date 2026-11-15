//! The 928-byte program block (memory-map.md §2) and its paged transfer (§1).

use std::fmt;

pub const PROGRAM_SIZE: usize = 928;
pub const PAGE_SIZE: usize = 50;
pub const PAGE_COUNT: usize = 19;
pub const INPUT_COUNT: usize = 2;
pub const OUTPUT_COUNT: usize = 6;
pub const INPUT_PEQ_BANDS: usize = 8;
pub const OUTPUT_PEQ_BANDS: usize = 9;
pub const GEQ_BANDS: usize = 31;
pub const NAME_LEN: usize = 8;
pub const PROGRAM_NAME_LEN: usize = 14;
/// Absolute offsets of the channel blocks.
pub const INPUT_BASE: usize = 0x010;
pub const INPUT_STRIDE: usize = 0x8C;
pub const OUTPUT_BASE: usize = 0x128;
pub const OUTPUT_STRIDE: usize = 0x64;
pub const GLOBAL_BASE: usize = 0x380;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramError {
    /// Fewer than [`PROGRAM_SIZE`] bytes were given.
    TooShort(usize),
    /// Page number outside 0..19.
    BadPage(u8),
    /// A page reply carried more than [`PAGE_SIZE`] bytes.
    PageTooLong(usize),
    /// Not a native program file (wrong magic or size).
    BadFile,
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::TooShort(n) => write!(f, "program needs {PROGRAM_SIZE} bytes, got {n}"),
            ProgramError::BadPage(p) => write!(f, "page {p} is outside 0..{PAGE_COUNT}"),
            ProgramError::PageTooLong(n) => write!(f, "page carries {n} bytes, max {PAGE_SIZE}"),
            ProgramError::BadFile => write!(f, "not a DSP 206 program file"),
        }
    }
}

impl std::error::Error for ProgramError {}

/// One parametric EQ band (memory-map.md §2.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeqBand {
    pub gain: u16,
    pub freq: u16,
    pub q: u8,
    pub kind: u8,
}

/// High-pass or low-pass filter settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CutFilter {
    pub freq: u16,
    pub slope: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gate {
    pub attack: u16,
    pub hold: u16,
    pub release: u16,
    pub threshold: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Compressor {
    pub ratio: u16,
    pub attack: u16,
    pub release: u16,
    pub knee: u16,
    pub threshold: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limiter {
    pub attack: u16,
    pub release: u16,
    pub knee: u16,
    pub threshold: u16,
}

/// Input channel block, 140 bytes (memory-map.md §2.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputChannel {
    pub name: [u8; NAME_LEN],
    pub gate: Gate,
    pub geq: [u16; GEQ_BANDS],
    pub peq: [PeqBand; INPUT_PEQ_BANDS],
    pub high_pass: CutFilter,
    pub low_pass: CutFilter,
    pub gain: u16,
    pub polarity: u16,
    pub delay: u16,
    pub link: u16,
}

/// Output channel block, 100 bytes (memory-map.md §2.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputChannel {
    pub name: [u8; NAME_LEN],
    /// Bit 0 = InA, bit 1 = InB.
    pub routing: u16,
    /// Mix gain from InA and InB.
    pub mix: [u16; INPUT_COUNT],
    pub high_pass: CutFilter,
    pub low_pass: CutFilter,
    pub peq: [PeqBand; OUTPUT_PEQ_BANDS],
    pub compressor: Compressor,
    pub limiter: Limiter,
    pub gain: u16,
    pub polarity: u16,
    pub delay: u16,
    pub link: u16,
}

/// Global masks, 32 bytes (memory-map.md §2.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Globals {
    pub input_mute: u16,
    pub output_mute: u16,
    pub input_peq_band_bypass: [u16; INPUT_COUNT],
    pub output_peq_band_bypass: [u16; OUTPUT_COUNT],
    pub unknown_394: u16,
    pub unknown_396: u16,
    pub delay_unit: u16,
    pub input_peq_bypass: u16,
    pub output_peq_bypass: u16,
    pub geq_bypass: u16,
}

/// The whole program block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    /// Bytes 0..2, meaning unknown (observed `ff 00`).
    pub header: [u8; 2],
    pub name: [u8; PROGRAM_NAME_LEN],
    pub inputs: [InputChannel; INPUT_COUNT],
    pub outputs: [OutputChannel; OUTPUT_COUNT],
    pub globals: Globals,
}

/// ASCII text of a NUL/space padded name field.
pub fn name_text(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end])
        .trim_end()
        .to_string()
}

/// Encode a name into a fixed field, NUL padded, non-ASCII replaced by `?`.
pub fn name_field<const N: usize>(text: &str) -> [u8; N] {
    let mut out = [0u8; N];
    for (slot, c) in out.iter_mut().zip(text.chars()) {
        *slot = if c.is_ascii() && !c.is_ascii_control() {
            c as u8
        } else {
            b'?'
        };
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.b[self.p];
        self.p += 1;
        v
    }
    fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes([self.b[self.p], self.b[self.p + 1]]);
        self.p += 2;
        v
    }
    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        out.copy_from_slice(&self.b[self.p..self.p + N]);
        self.p += N;
        out
    }
    fn words<const N: usize>(&mut self) -> [u16; N] {
        let mut out = [0u16; N];
        for w in out.iter_mut() {
            *w = self.u16();
        }
        out
    }
    fn peq<const N: usize>(&mut self) -> [PeqBand; N] {
        let mut out = [PeqBand::default(); N];
        for band in out.iter_mut() {
            *band = PeqBand {
                gain: self.u16(),
                freq: self.u16(),
                q: self.u8(),
                kind: self.u8(),
            };
        }
        out
    }
    fn cut_filters(&mut self) -> (CutFilter, CutFilter) {
        let (hp_freq, lp_freq) = (self.u16(), self.u16());
        let (hp_slope, lp_slope) = (self.u8(), self.u8());
        (
            CutFilter {
                freq: hp_freq,
                slope: hp_slope,
            },
            CutFilter {
                freq: lp_freq,
                slope: lp_slope,
            },
        )
    }
}

#[derive(Default)]
struct Writer {
    b: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.b.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.b.extend_from_slice(&v.to_le_bytes());
    }
    fn words(&mut self, v: &[u16]) {
        v.iter().for_each(|w| self.u16(*w));
    }
    fn peq(&mut self, bands: &[PeqBand]) {
        for b in bands {
            self.u16(b.gain);
            self.u16(b.freq);
            self.u8(b.q);
            self.u8(b.kind);
        }
    }
    fn cut_filters(&mut self, hp: &CutFilter, lp: &CutFilter) {
        self.u16(hp.freq);
        self.u16(lp.freq);
        self.u8(hp.slope);
        self.u8(lp.slope);
    }
}

impl Program {
    /// Parse a program block. Extra trailing bytes (page padding) are ignored.
    pub fn from_bytes(bytes: &[u8]) -> Result<Program, ProgramError> {
        if bytes.len() < PROGRAM_SIZE {
            return Err(ProgramError::TooShort(bytes.len()));
        }
        let mut r = Reader {
            b: &bytes[..PROGRAM_SIZE],
            p: 0,
        };
        let header = r.bytes();
        let name = r.bytes();
        let mut inputs: [InputChannel; INPUT_COUNT] = Default::default();
        for ch in inputs.iter_mut() {
            ch.name = r.bytes();
            ch.gate = Gate {
                attack: r.u16(),
                hold: r.u16(),
                release: r.u16(),
                threshold: r.u16(),
            };
            ch.geq = r.words();
            ch.peq = r.peq();
            (ch.high_pass, ch.low_pass) = r.cut_filters();
            [ch.gain, ch.polarity, ch.delay, ch.link] = r.words();
        }
        let mut outputs: [OutputChannel; OUTPUT_COUNT] = Default::default();
        for ch in outputs.iter_mut() {
            ch.name = r.bytes();
            ch.routing = r.u16();
            ch.mix = r.words();
            (ch.high_pass, ch.low_pass) = r.cut_filters();
            ch.peq = r.peq();
            let [ratio, attack, release, knee, threshold] = r.words();
            ch.compressor = Compressor {
                ratio,
                attack,
                release,
                knee,
                threshold,
            };
            let [attack, release, knee, threshold] = r.words();
            ch.limiter = Limiter {
                attack,
                release,
                knee,
                threshold,
            };
            [ch.gain, ch.polarity, ch.delay, ch.link] = r.words();
        }
        let globals = Globals {
            input_mute: r.u16(),
            output_mute: r.u16(),
            input_peq_band_bypass: r.words(),
            output_peq_band_bypass: r.words(),
            unknown_394: r.u16(),
            unknown_396: r.u16(),
            delay_unit: r.u16(),
            input_peq_bypass: r.u16(),
            output_peq_bypass: r.u16(),
            geq_bypass: r.u16(),
        };
        debug_assert_eq!(r.p, PROGRAM_SIZE);
        Ok(Program {
            header,
            name,
            inputs,
            outputs,
            globals,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer {
            b: Vec::with_capacity(PROGRAM_SIZE),
        };
        w.b.extend_from_slice(&self.header);
        w.b.extend_from_slice(&self.name);
        for ch in &self.inputs {
            w.b.extend_from_slice(&ch.name);
            let g = &ch.gate;
            w.words(&[g.attack, g.hold, g.release, g.threshold]);
            w.words(&ch.geq);
            w.peq(&ch.peq);
            w.cut_filters(&ch.high_pass, &ch.low_pass);
            w.words(&[ch.gain, ch.polarity, ch.delay, ch.link]);
        }
        for ch in &self.outputs {
            w.b.extend_from_slice(&ch.name);
            w.u16(ch.routing);
            w.words(&ch.mix);
            w.cut_filters(&ch.high_pass, &ch.low_pass);
            w.peq(&ch.peq);
            let c = &ch.compressor;
            w.words(&[c.ratio, c.attack, c.release, c.knee, c.threshold]);
            let l = &ch.limiter;
            w.words(&[l.attack, l.release, l.knee, l.threshold]);
            w.words(&[ch.gain, ch.polarity, ch.delay, ch.link]);
        }
        let g = &self.globals;
        w.words(&[g.input_mute, g.output_mute]);
        w.words(&g.input_peq_band_bypass);
        w.words(&g.output_peq_band_bypass);
        w.words(&[
            g.unknown_394,
            g.unknown_396,
            g.delay_unit,
            g.input_peq_bypass,
            g.output_peq_bypass,
            g.geq_bypass,
        ]);
        w.b
    }

    pub fn name_text(&self) -> String {
        name_text(&self.name)
    }

    /// Split the block into the 19 pages of 50 bytes sent by `0x24` replies.
    pub fn to_pages(&self) -> Vec<[u8; PAGE_SIZE]> {
        let mut bytes = self.to_bytes();
        bytes.resize(PAGE_SIZE * PAGE_COUNT, 0);
        bytes
            .chunks_exact(PAGE_SIZE)
            .map(|c| {
                let mut page = [0u8; PAGE_SIZE];
                page.copy_from_slice(c);
                page
            })
            .collect()
    }
}

/// Collects `0x24` page replies until the program is complete.
#[derive(Clone, Debug)]
pub struct ProgramAssembler {
    raw: Vec<u8>,
    received: [bool; PAGE_COUNT],
}

impl Default for ProgramAssembler {
    fn default() -> Self {
        Self {
            raw: vec![0; PAGE_SIZE * PAGE_COUNT],
            received: [false; PAGE_COUNT],
        }
    }
}

impl ProgramAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store one page (`data` excludes the leading page number).
    pub fn insert(&mut self, page: u8, data: &[u8]) -> Result<(), ProgramError> {
        let p = page as usize;
        if p >= PAGE_COUNT {
            return Err(ProgramError::BadPage(page));
        }
        if data.len() > PAGE_SIZE {
            return Err(ProgramError::PageTooLong(data.len()));
        }
        self.raw[p * PAGE_SIZE..p * PAGE_SIZE + data.len()].copy_from_slice(data);
        self.received[p] = true;
        Ok(())
    }

    pub fn missing_pages(&self) -> Vec<u8> {
        (0..PAGE_COUNT as u8)
            .filter(|p| !self.received[*p as usize])
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.received.iter().all(|r| *r)
    }

    /// Raw page buffer, zeros where pages are missing.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The program, or `None` while pages are missing.
    pub fn program(&self) -> Option<Program> {
        if self.is_complete() {
            Program::from_bytes(&self.raw).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random bytes (xorshift), no external crate.
    fn noise(seed: u32, n: usize) -> Vec<u8> {
        let mut x = seed.max(1);
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    #[test]
    fn layout_covers_every_byte() {
        // from_bytes/to_bytes must be lossless for any content: the struct
        // describes all 928 bytes, in order.
        for seed in 1..20 {
            let bytes = noise(seed, PROGRAM_SIZE);
            assert_eq!(Program::from_bytes(&bytes).unwrap().to_bytes(), bytes);
        }
        assert_eq!(Program::default().to_bytes().len(), PROGRAM_SIZE);
    }

    #[test]
    fn fields_sit_at_documented_offsets() {
        // memory-map.md §2: write a marker through each field, find it in the bytes.
        let mut p = Program {
            name: name_field("Default Preset"),
            ..Default::default()
        };
        p.inputs[1].gain = 0xA1B2;
        p.inputs[0].geq[30] = 0x0102;
        p.inputs[0].peq[7] = PeqBand {
            gain: 0x1111,
            freq: 0x2222,
            q: 0x33,
            kind: 0x44,
        };
        p.inputs[0].high_pass.slope = 0x55;
        p.outputs[0].routing = 0x0201;
        p.outputs[0].compressor.threshold = 0xC3D4;
        p.outputs[5].link = 0xBEEF;
        p.globals.input_mute = 0x0003;
        p.globals.output_mute = 0x003F;
        p.globals.geq_bypass = 0x7777;
        let b = p.to_bytes();
        let w = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        assert_eq!(&b[0x002..0x010], b"Default Preset");
        assert_eq!(w(INPUT_BASE + INPUT_STRIDE + 0x84), 0xA1B2); // InB gain at 0x120
        assert_eq!(w(0x020 + 30 * 2), 0x0102); // InA GEQ band 31
        assert_eq!(w(0x05E + 7 * 6), 0x1111); // InA PEQ 8 gain
        assert_eq!(w(0x05E + 7 * 6 + 2), 0x2222); // InA PEQ 8 freq
        assert_eq!(b[0x05E + 7 * 6 + 4], 0x33);
        assert_eq!(b[0x05E + 7 * 6 + 5], 0x44);
        assert_eq!(b[0x092], 0x55); // InA HPF slope
        assert_eq!(w(0x130), 0x0201); // Out1 routing
        assert_eq!(w(0x17A), 0xC3D4); // Out1 compressor threshold
        assert_eq!(w(OUTPUT_BASE + 5 * OUTPUT_STRIDE + 0x62), 0xBEEF); // Out6 link at 0x37E
        assert_eq!(w(0x380), 0x0003);
        assert_eq!(w(0x382), 0x003F);
        assert_eq!(w(0x39E), 0x7777);
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(
            Program::from_bytes(&[0; 10]),
            Err(ProgramError::TooShort(10))
        );
        // Page buffers are 950 bytes: the padding is ignored.
        assert!(Program::from_bytes(&[0; PAGE_SIZE * PAGE_COUNT]).is_ok());
    }

    #[test]
    fn pages_round_trip_through_assembler() {
        let program = Program::from_bytes(&noise(7, PROGRAM_SIZE)).unwrap();
        let pages = program.to_pages();
        assert_eq!(pages.len(), PAGE_COUNT);
        let mut asm = ProgramAssembler::new();
        for (i, page) in pages.iter().enumerate().rev() {
            assert!(asm.program().is_none());
            asm.insert(i as u8, page).unwrap();
        }
        assert!(asm.missing_pages().is_empty());
        assert_eq!(asm.program().unwrap(), program);
        assert_eq!(asm.insert(19, &[]), Err(ProgramError::BadPage(19)));
        assert_eq!(asm.insert(0, &[0; 51]), Err(ProgramError::PageTooLong(51)));
    }

    #[test]
    fn names() {
        assert_eq!(name_text(b"InA\0\0\0\0\0"), "InA");
        assert_eq!(name_text(b"Out6    "), "Out6");
        assert_eq!(name_field::<8>("Sub Left too long"), *b"Sub Left");
        assert_eq!(name_field::<4>("é"), *b"?\0\0\0");
    }
}

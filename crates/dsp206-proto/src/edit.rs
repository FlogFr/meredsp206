//! Editing a [`Program`]: factory defaults, applying setter requests locally,
//! generating the requests that reproduce a program on a device, and the
//! native program file format.

use crate::command::{Channel, Request};
use crate::program::*;

/// Native program file: magic + the 928-byte block.
pub const FILE_MAGIC: &[u8; 8] = b"DSP206P\x01";

/// Mutable view of the fields every channel has.
pub struct StripMut<'a> {
    pub name: &'a mut [u8; NAME_LEN],
    pub gain: &'a mut u16,
    pub polarity: &'a mut u16,
    pub delay: &'a mut u16,
    pub link: &'a mut u16,
    pub high_pass: &'a mut CutFilter,
    pub low_pass: &'a mut CutFilter,
    pub peq: &'a mut [PeqBand],
}

/// Read-only view of the fields every channel has.
#[derive(Clone, Copy)]
pub struct Strip<'a> {
    pub name: &'a [u8; NAME_LEN],
    pub gain: u16,
    pub polarity: u16,
    pub delay: u16,
    pub link: u16,
    pub high_pass: CutFilter,
    pub low_pass: CutFilter,
    pub peq: &'a [PeqBand],
}

fn set_bit(word: &mut u16, bit: u8, on: bool) {
    if on {
        *word |= 1 << bit;
    } else {
        *word &= !(1 << bit);
    }
}

fn bit(word: u16, bit: u8) -> bool {
    word & (1 << bit) != 0
}

/// Bit index of a channel inside its domain mask (inputs 0–1, outputs 0–5).
pub fn domain_bit(ch: Channel) -> u8 {
    if ch.is_input() {
        ch.index()
    } else {
        ch.index() - 2
    }
}

fn peq_default(freq: u16) -> PeqBand {
    PeqBand {
        gain: 120,
        freq,
        q: 35,
        kind: 0,
    }
}

impl Program {
    /// The factory "Default Preset" (memory-map.md §5). Pages that were not
    /// captured are assumed to follow the same pattern; output routing of the
    /// even outputs (InB) is an assumption.
    pub fn factory_default() -> Program {
        let mut p = Program {
            header: [0xff, 0x00],
            name: name_field("Default Preset"),
            ..Default::default()
        };
        let in_freqs = [41, 71, 101, 140, 170, 200, 240, 270];
        for (k, ch) in p.inputs.iter_mut().enumerate() {
            ch.name = name_field(["InA", "InB"][k]);
            ch.gate = Gate {
                attack: 0,
                hold: 99,
                release: 99,
                threshold: 0,
            };
            ch.geq = [120; GEQ_BANDS];
            ch.peq = in_freqs.map(peq_default);
            ch.high_pass = CutFilter { freq: 0, slope: 0 };
            ch.low_pass = CutFilter {
                freq: 300,
                slope: 0,
            };
            ch.gain = 280;
            ch.link = 1 << k;
        }
        let out_freqs = [31, 63, 95, 127, 158, 190, 222, 253, 285];
        for (k, ch) in p.outputs.iter_mut().enumerate() {
            ch.name = name_field(&format!("Out{}", k + 1));
            ch.routing = if k % 2 == 0 { 1 } else { 2 };
            ch.mix = [280, 280];
            ch.high_pass = CutFilter { freq: 0, slope: 0 };
            ch.low_pass = CutFilter {
                freq: 300,
                slope: 0,
            };
            ch.peq = out_freqs.map(peq_default);
            ch.compressor = Compressor {
                ratio: 0,
                attack: 49,
                release: 499,
                knee: 0,
                threshold: 220,
            };
            ch.limiter = Limiter {
                attack: 49,
                release: 499,
                knee: 0,
                threshold: 220,
            };
            ch.gain = 280;
            ch.link = 1 << k;
        }
        p
    }

    pub fn strip(&self, ch: Channel) -> Strip<'_> {
        if ch.is_input() {
            let c = &self.inputs[ch.index() as usize];
            Strip {
                name: &c.name,
                gain: c.gain,
                polarity: c.polarity,
                delay: c.delay,
                link: c.link,
                high_pass: c.high_pass,
                low_pass: c.low_pass,
                peq: &c.peq,
            }
        } else {
            let c = &self.outputs[ch.index() as usize - 2];
            Strip {
                name: &c.name,
                gain: c.gain,
                polarity: c.polarity,
                delay: c.delay,
                link: c.link,
                high_pass: c.high_pass,
                low_pass: c.low_pass,
                peq: &c.peq,
            }
        }
    }

    pub fn strip_mut(&mut self, ch: Channel) -> StripMut<'_> {
        if ch.is_input() {
            let c = &mut self.inputs[ch.index() as usize];
            StripMut {
                name: &mut c.name,
                gain: &mut c.gain,
                polarity: &mut c.polarity,
                delay: &mut c.delay,
                link: &mut c.link,
                high_pass: &mut c.high_pass,
                low_pass: &mut c.low_pass,
                peq: &mut c.peq,
            }
        } else {
            let c = &mut self.outputs[ch.index() as usize - 2];
            StripMut {
                name: &mut c.name,
                gain: &mut c.gain,
                polarity: &mut c.polarity,
                delay: &mut c.delay,
                link: &mut c.link,
                high_pass: &mut c.high_pass,
                low_pass: &mut c.low_pass,
                peq: &mut c.peq,
            }
        }
    }

    pub fn channel_name(&self, ch: Channel) -> String {
        name_text(self.strip(ch).name)
    }

    pub fn is_muted(&self, ch: Channel) -> bool {
        let mask = if ch.is_input() {
            self.globals.input_mute
        } else {
            self.globals.output_mute
        };
        bit(mask, domain_bit(ch))
    }

    pub fn peq_bypassed(&self, ch: Channel) -> bool {
        let mask = if ch.is_input() {
            self.globals.input_peq_bypass
        } else {
            self.globals.output_peq_bypass
        };
        bit(mask, domain_bit(ch))
    }

    pub fn peq_band_bypassed(&self, ch: Channel, band: u8) -> bool {
        let k = domain_bit(ch) as usize;
        let mask = if ch.is_input() {
            self.globals.input_peq_band_bypass[k]
        } else {
            self.globals.output_peq_band_bypass[k]
        };
        bit(mask, band)
    }

    pub fn geq_bypassed(&self, ch: Channel) -> bool {
        ch.is_input() && bit(self.globals.geq_bypass, domain_bit(ch))
    }

    /// Channels that follow `ch`: every channel of the same domain whose bit is
    /// set in `ch`'s link mask, always including `ch` itself.
    pub fn linked_channels(&self, ch: Channel) -> Vec<Channel> {
        let mask = self.strip(ch).link;
        let (base, count) = if ch.is_input() { (0, 2) } else { (2, 6) };
        let mut out: Vec<Channel> = (0..count)
            .filter(|i| bit(mask, *i))
            .filter_map(|i| Channel::new(base + i))
            .collect();
        if !out.contains(&ch) {
            out.insert(0, ch);
        }
        out
    }

    /// Apply a setter to the local model. Processing setters also update the
    /// linked channels, as the original editor does. Returns `true` when the
    /// model changed. Reads, preset commands and invalid targets change nothing.
    pub fn apply(&mut self, req: &Request) -> bool {
        let before = self.clone();
        self.apply_inner(req);
        *self != before
    }

    fn apply_inner(&mut self, req: &Request) {
        use Request::*;
        match req {
            SetGain { ch, raw } => {
                for c in self.linked_channels(*ch) {
                    *self.strip_mut(c).gain = *raw;
                }
            }
            SetPolarity { ch, inverted } => {
                for c in self.linked_channels(*ch) {
                    *self.strip_mut(c).polarity = u16::from(*inverted);
                }
            }
            SetDelay { ch, raw } => {
                for c in self.linked_channels(*ch) {
                    *self.strip_mut(c).delay = *raw;
                }
            }
            SetMute { ch, on } => {
                for c in self.linked_channels(*ch) {
                    let g = &mut self.globals;
                    let mask = if c.is_input() {
                        &mut g.input_mute
                    } else {
                        &mut g.output_mute
                    };
                    set_bit(mask, domain_bit(c), *on);
                }
            }
            SetDelayUnit(u) => self.globals.delay_unit = u16::from(*u),
            SetPeqBand {
                ch,
                band,
                value,
                bypass,
            } => {
                for c in self.linked_channels(*ch) {
                    let s = self.strip_mut(c);
                    let Some(slot) = s.peq.get_mut(*band as usize) else {
                        return;
                    };
                    *slot = *value;
                    let k = domain_bit(c) as usize;
                    let g = &mut self.globals;
                    let mask = if c.is_input() {
                        &mut g.input_peq_band_bypass[k]
                    } else {
                        &mut g.output_peq_band_bypass[k]
                    };
                    set_bit(mask, *band, *bypass);
                }
            }
            SetPeqBypass { ch, on } => {
                for c in self.linked_channels(*ch) {
                    let g = &mut self.globals;
                    let mask = if c.is_input() {
                        &mut g.input_peq_bypass
                    } else {
                        &mut g.output_peq_bypass
                    };
                    set_bit(mask, domain_bit(c), *on);
                }
            }
            SetGeqBand { ch, band, gain } if ch.is_input() && (*band as usize) < GEQ_BANDS => {
                for c in self.linked_channels(*ch) {
                    self.inputs[c.index() as usize].geq[*band as usize] = *gain;
                }
            }
            SetGeqBypass { ch, on } if ch.is_input() => {
                for c in self.linked_channels(*ch) {
                    set_bit(&mut self.globals.geq_bypass, domain_bit(c), *on);
                }
            }
            SetHighPass { ch, filter } => {
                for c in self.linked_channels(*ch) {
                    *self.strip_mut(c).high_pass = *filter;
                }
            }
            SetLowPass { ch, filter } => {
                for c in self.linked_channels(*ch) {
                    *self.strip_mut(c).low_pass = *filter;
                }
            }
            SetGate { ch, gate } if ch.is_input() => {
                for c in self.linked_channels(*ch) {
                    self.inputs[c.index() as usize].gate = *gate;
                }
            }
            SetCompressor { ch, comp } if !ch.is_input() => {
                for c in self.linked_channels(*ch) {
                    self.outputs[c.index() as usize - 2].compressor = *comp;
                }
            }
            SetLimiter { ch, limiter } if !ch.is_input() => {
                for c in self.linked_channels(*ch) {
                    self.outputs[c.index() as usize - 2].limiter = *limiter;
                }
            }
            SetRouting { ch, mask } if !ch.is_input() => {
                self.outputs[ch.index() as usize - 2].routing = u16::from(*mask);
            }
            SetMixGain { ch, input, raw } if !ch.is_input() && *input < 2 => {
                self.outputs[ch.index() as usize - 2].mix[*input as usize] = *raw;
            }
            SetLink { ch, mask } => *self.strip_mut(*ch).link = u16::from(*mask),
            SetChannelName { ch, name } => *self.strip_mut(*ch).name = *name,
            CopyChannel { src, dst } if src.is_input() == dst.is_input() && src != dst => {
                self.copy_channel(*src, *dst);
            }
            _ => {}
        }
    }

    /// Copy every processing setting of `src` to `dst` (same domain), keeping
    /// the destination's name, link mask and routing.
    fn copy_channel(&mut self, src: Channel, dst: Channel) {
        if src.is_input() {
            let mut c = self.inputs[src.index() as usize].clone();
            let d = &self.inputs[dst.index() as usize];
            (c.name, c.link) = (d.name, d.link);
            self.inputs[dst.index() as usize] = c;
        } else {
            let mut c = self.outputs[src.index() as usize - 2].clone();
            let d = &self.outputs[dst.index() as usize - 2];
            (c.name, c.link, c.routing, c.mix) = (d.name, d.link, d.routing, d.mix);
            self.outputs[dst.index() as usize - 2] = c;
        }
        // Flags stored in the global masks.
        let (s, d) = (domain_bit(src), domain_bit(dst));
        let copy = |mask: &mut u16| {
            let on = bit(*mask, s);
            set_bit(mask, d, on);
        };
        let g = &mut self.globals;
        if src.is_input() {
            copy(&mut g.input_mute);
            copy(&mut g.input_peq_bypass);
            copy(&mut g.geq_bypass);
            g.input_peq_band_bypass[d as usize] = g.input_peq_band_bypass[s as usize];
        } else {
            copy(&mut g.output_mute);
            copy(&mut g.output_peq_bypass);
            g.output_peq_band_bypass[d as usize] = g.output_peq_band_bypass[s as usize];
        }
    }

    /// Setter requests that reproduce this program on a device, link masks last
    /// so earlier setters do not propagate unexpectedly.
    pub fn sync_requests(&self) -> Vec<Request> {
        let mut out = vec![Request::SetDelayUnit(self.globals.delay_unit as u8)];
        let mut links = Vec::new();
        for ch in Channel::all() {
            let s = self.strip(ch);
            out.push(Request::SetChannelName { ch, name: *s.name });
            out.push(Request::SetGain { ch, raw: s.gain });
            out.push(Request::SetMute {
                ch,
                on: self.is_muted(ch),
            });
            out.push(Request::SetPolarity {
                ch,
                inverted: s.polarity != 0,
            });
            out.push(Request::SetDelay { ch, raw: s.delay });
            out.push(Request::SetHighPass {
                ch,
                filter: s.high_pass,
            });
            out.push(Request::SetLowPass {
                ch,
                filter: s.low_pass,
            });
            for (band, value) in s.peq.iter().enumerate() {
                out.push(Request::SetPeqBand {
                    ch,
                    band: band as u8,
                    value: *value,
                    bypass: self.peq_band_bypassed(ch, band as u8),
                });
            }
            out.push(Request::SetPeqBypass {
                ch,
                on: self.peq_bypassed(ch),
            });
            if ch.is_input() {
                let c = &self.inputs[ch.index() as usize];
                out.push(Request::SetGate { ch, gate: c.gate });
                for (band, gain) in c.geq.iter().enumerate() {
                    out.push(Request::SetGeqBand {
                        ch,
                        band: band as u8,
                        gain: *gain,
                    });
                }
                out.push(Request::SetGeqBypass {
                    ch,
                    on: self.geq_bypassed(ch),
                });
            } else {
                let c = &self.outputs[ch.index() as usize - 2];
                out.push(Request::SetRouting {
                    ch,
                    mask: c.routing as u8,
                });
                for (input, raw) in c.mix.iter().enumerate() {
                    out.push(Request::SetMixGain {
                        ch,
                        input: input as u8,
                        raw: *raw,
                    });
                }
                out.push(Request::SetCompressor {
                    ch,
                    comp: c.compressor,
                });
                out.push(Request::SetLimiter {
                    ch,
                    limiter: c.limiter,
                });
            }
            links.push(Request::SetLink {
                ch,
                mask: s.link as u8,
            });
        }
        out.extend(links);
        out
    }

    /// The setters needed to turn a device holding `self` into `target`.
    pub fn diff_requests(&self, target: &Program) -> Vec<Request> {
        let current = self.sync_requests();
        target
            .sync_requests()
            .into_iter()
            .filter(|r| !current.contains(r))
            .collect()
    }

    /// Serialize to the native `.dsp206` file format.
    pub fn to_file(&self) -> Vec<u8> {
        let mut out = FILE_MAGIC.to_vec();
        out.extend(self.to_bytes());
        out
    }

    pub fn from_file(bytes: &[u8]) -> Result<Program, ProgramError> {
        let body = bytes
            .strip_prefix(FILE_MAGIC.as_slice())
            .ok_or(ProgramError::BadFile)?;
        if body.len() != PROGRAM_SIZE {
            return Err(ProgramError::BadFile);
        }
        Program::from_bytes(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units;

    fn ch(i: u8) -> Channel {
        Channel::new(i).unwrap()
    }

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

    /// A random program with valid (own-bit) link masks.
    fn random_program(seed: u32) -> Program {
        let mut p = Program::from_bytes(&noise(seed, PROGRAM_SIZE)).unwrap();
        for c in Channel::all() {
            let s = p.strip_mut(c);
            *s.link = 1 << domain_bit(c);
            *s.polarity &= 1;
            s.name.iter_mut().for_each(|b| *b = b'A' + *b % 26);
        }
        for o in p.outputs.iter_mut() {
            o.routing &= 0xff;
        }
        // Masks only hold bits that a setter can address.
        let g = &mut p.globals;
        g.delay_unit &= 0xff;
        g.input_mute &= 0b11;
        g.input_peq_bypass &= 0b11;
        g.geq_bypass &= 0b11;
        g.output_mute &= 0b11_1111;
        g.output_peq_bypass &= 0b11_1111;
        g.input_peq_band_bypass.iter_mut().for_each(|m| *m &= 0xff);
        g.output_peq_band_bypass
            .iter_mut()
            .for_each(|m| *m &= 0x1ff);
        p
    }

    #[test]
    fn factory_default_is_sane() {
        let p = Program::factory_default();
        assert_eq!(p.name_text(), "Default Preset");
        assert_eq!(p.channel_name(ch(0)), "InA");
        assert_eq!(p.channel_name(ch(7)), "Out6");
        for c in Channel::all() {
            assert_eq!(units::gain_db(p.strip(c).gain), 0.0);
            assert!(!p.is_muted(c));
            assert_eq!(p.linked_channels(c), vec![c]);
        }
    }

    #[test]
    fn mute_and_bypass_bits() {
        let mut p = Program::factory_default();
        assert!(p.apply(&Request::SetMute {
            ch: ch(7),
            on: true
        }));
        assert_eq!(p.globals.output_mute, 0b10_0000);
        assert!(p.is_muted(ch(7)) && !p.is_muted(ch(6)));
        p.apply(&Request::SetMute {
            ch: ch(1),
            on: true,
        });
        assert_eq!(p.globals.input_mute, 0b10);
        p.apply(&Request::SetPeqBypass {
            ch: ch(2),
            on: true,
        });
        assert_eq!(p.globals.output_peq_bypass, 1);
        p.apply(&Request::SetGeqBypass {
            ch: ch(0),
            on: true,
        });
        assert_eq!(p.globals.geq_bypass, 1);
        let band = p.outputs[1].peq[8];
        p.apply(&Request::SetPeqBand {
            ch: ch(3),
            band: 8,
            value: band,
            bypass: true,
        });
        assert_eq!(p.globals.output_peq_band_bypass[1], 1 << 8);
        assert!(p.peq_band_bypassed(ch(3), 8));
        // Setting the same value again changes nothing.
        assert!(!p.apply(&Request::SetMute {
            ch: ch(7),
            on: true
        }));
    }

    #[test]
    fn linked_channels_follow_processing_but_not_names() {
        let mut p = Program::factory_default();
        // Link Out1 and Out2 (bits 0 and 1 on both).
        p.apply(&Request::SetLink {
            ch: ch(2),
            mask: 0b11,
        });
        p.apply(&Request::SetLink {
            ch: ch(3),
            mask: 0b11,
        });
        assert_eq!(p.linked_channels(ch(2)), vec![ch(2), ch(3)]);
        p.apply(&Request::SetGain {
            ch: ch(2),
            raw: 200,
        });
        assert_eq!(
            (p.outputs[0].gain, p.outputs[1].gain, p.outputs[2].gain),
            (200, 200, 280)
        );
        p.apply(&Request::SetMute {
            ch: ch(3),
            on: true,
        });
        assert_eq!(p.globals.output_mute, 0b11);
        p.apply(&Request::SetChannelName {
            ch: ch(2),
            name: name_field("Sub"),
        });
        assert_eq!(
            (p.channel_name(ch(2)), p.channel_name(ch(3))),
            ("Sub".into(), "Out2".into())
        );
    }

    #[test]
    fn invalid_targets_are_ignored() {
        let mut p = Program::factory_default();
        let gate = Gate::default();
        assert!(!p.apply(&Request::SetGate { ch: ch(2), gate }));
        assert!(!p.apply(&Request::SetGeqBand {
            ch: ch(3),
            band: 0,
            gain: 0
        }));
        assert!(!p.apply(&Request::SetGeqBand {
            ch: ch(0),
            band: 31,
            gain: 0
        }));
        assert!(!p.apply(&Request::SetPeqBand {
            ch: ch(0),
            band: 8,
            value: PeqBand::default(),
            bypass: false
        }));
        assert!(!p.apply(&Request::SetRouting { ch: ch(0), mask: 3 }));
        assert!(!p.apply(&Request::CopyChannel {
            src: ch(0),
            dst: ch(2)
        }));
        assert!(!p.apply(&Request::Identify));
    }

    #[test]
    fn sync_requests_reproduce_any_program() {
        for seed in 1..10 {
            let target = random_program(seed);
            let mut p = Program::factory_default();
            for r in target.sync_requests() {
                p.apply(&r);
            }
            // Fields without a setter: header, program name, unknown words.
            p.header = target.header;
            p.name = target.name;
            p.globals.unknown_394 = target.globals.unknown_394;
            p.globals.unknown_396 = target.globals.unknown_396;
            assert_eq!(p, target, "seed {seed}");
        }
    }

    #[test]
    fn diff_contains_only_changes() {
        let a = Program::factory_default();
        let mut b = a.clone();
        assert!(a.diff_requests(&b).is_empty());
        b.apply(&Request::SetGain {
            ch: ch(4),
            raw: 300,
        });
        b.apply(&Request::SetMute {
            ch: ch(0),
            on: true,
        });
        assert_eq!(
            a.diff_requests(&b),
            vec![
                Request::SetMute {
                    ch: ch(0),
                    on: true
                },
                Request::SetGain {
                    ch: ch(4),
                    raw: 300
                },
            ]
        );
    }

    #[test]
    fn copy_channel_keeps_identity_of_destination() {
        let mut p = Program::factory_default();
        p.apply(&Request::SetGain {
            ch: ch(2),
            raw: 111,
        });
        p.apply(&Request::SetMute {
            ch: ch(2),
            on: true,
        });
        p.apply(&Request::SetRouting { ch: ch(2), mask: 3 });
        assert!(p.apply(&Request::CopyChannel {
            src: ch(2),
            dst: ch(5)
        }));
        let (src, dst) = (&p.outputs[0], &p.outputs[3]);
        assert_eq!(dst.gain, 111);
        assert_eq!(dst.compressor, src.compressor);
        assert!(p.is_muted(ch(5)));
        assert_eq!(p.channel_name(ch(5)), "Out4");
        assert_eq!(p.outputs[3].routing, 2); // routing not copied
    }

    #[test]
    fn file_round_trip() {
        let p = random_program(3);
        let bytes = p.to_file();
        assert_eq!(&bytes[..8], FILE_MAGIC);
        assert_eq!(Program::from_file(&bytes).unwrap(), p);
        assert_eq!(Program::from_file(&bytes[1..]), Err(ProgramError::BadFile));
        assert_eq!(
            Program::from_file(&bytes[..100]),
            Err(ProgramError::BadFile)
        );
    }
}

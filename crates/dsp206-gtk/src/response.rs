//! Frequency response of a channel for the EQ graph (display only).
//!
//! PEQ bands use RBJ biquads at 96 kHz. Crossover filters use analytic
//! Butterworth magnitudes; Linkwitz-Riley is a squared Butterworth and Bessel
//! is drawn with the Butterworth magnitude of the same order (approximation).

use std::f64::consts::PI;

use dsp206_proto::program::{CutFilter, PeqBand};
use dsp206_proto::units::{self, FilterFamily, PeqType};

const FS: f64 = 96_000.0;

/// Normalised biquad `b0 b1 b2 / a0 a1 a2`.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b: [f64; 3],
    a: [f64; 3],
}

impl Biquad {
    pub fn magnitude_db(&self, f: f64) -> f64 {
        let w = 2.0 * PI * f / FS;
        let eval = |k: &[f64; 3]| {
            let re = k[0] + k[1] * w.cos() + k[2] * (2.0 * w).cos();
            let im = -(k[1] * w.sin() + k[2] * (2.0 * w).sin());
            (re * re + im * im).sqrt()
        };
        20.0 * (eval(&self.b) / eval(&self.a)).max(1e-12).log10()
    }
}

/// RBJ cookbook biquad for a PEQ band.
pub fn peq_biquad(kind: PeqType, f0: f64, gain_db: f64, q: f64) -> Biquad {
    let a_lin = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * f0 / FS;
    let (c, s) = (w0.cos(), w0.sin());
    let alpha = s / (2.0 * q);
    let (b, a) = match kind {
        PeqType::Peak => (
            [1.0 + alpha * a_lin, -2.0 * c, 1.0 - alpha * a_lin],
            [1.0 + alpha / a_lin, -2.0 * c, 1.0 - alpha / a_lin],
        ),
        PeqType::LowShelf | PeqType::HighShelf => {
            let sq = 2.0 * a_lin.sqrt() * alpha;
            let sign = if kind == PeqType::LowShelf { 1.0 } else { -1.0 };
            let (ap, am) = (a_lin + 1.0, a_lin - 1.0);
            (
                [
                    a_lin * (ap - sign * am * c + sq),
                    sign * 2.0 * a_lin * (am - sign * ap * c),
                    a_lin * (ap - sign * am * c - sq),
                ],
                [
                    ap + sign * am * c + sq,
                    -sign * 2.0 * (am + sign * ap * c),
                    ap + sign * am * c - sq,
                ],
            )
        }
        PeqType::LowPass12 => (
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        ),
        PeqType::HighPass12 => (
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        ),
        PeqType::LowPass6 | PeqType::HighPass6 => {
            // Bilinear first-order sections.
            let k = (w0 / 2.0).tan();
            if kind == PeqType::LowPass6 {
                ([k, k, 0.0], [k + 1.0, k - 1.0, 0.0])
            } else {
                ([1.0, -1.0, 0.0], [k + 1.0, k - 1.0, 0.0])
            }
        }
        PeqType::Allpass1 | PeqType::Allpass2 => ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
    };
    Biquad { b, a }
}

/// Response of one stored PEQ band, or `None` when its raw values are invalid.
pub fn band_response(band: &PeqBand) -> Option<Biquad> {
    let kind = PeqType::from_raw(band.kind)?;
    let q = units::q_value(band.q)?;
    Some(peq_biquad(
        kind,
        units::freq_hz(band.freq),
        units::eq_gain_db(band.gain),
        q,
    ))
}

/// Crossover magnitude in dB (`high_pass` selects the filter side).
pub fn cut_db(filter: &CutFilter, high_pass: bool, f: f64) -> f64 {
    let Some((family, slope)) = units::SLOPES.get(filter.slope as usize) else {
        return 0.0;
    };
    let fc = units::freq_hz(filter.freq);
    let ratio = if high_pass { fc / f } else { f / fc };
    let (order, squared) = match family {
        FilterFamily::LinkwitzRiley => (f64::from(*slope) / 12.0, true),
        _ => (f64::from(*slope) / 6.0, false),
    };
    let bw = -10.0 * (1.0 + ratio.powf(2.0 * order)).log10();
    if squared {
        2.0 * bw
    } else {
        bw
    }
}

/// Approximate response of a GEQ band (1/3-octave peaking filter).
pub fn geq_band(f0: f64, gain_db: f64) -> Biquad {
    peq_biquad(PeqType::Peak, f0, gain_db, 4.32)
}

/// `n` log-spaced frequencies from 20 Hz to 20 kHz.
pub fn freq_axis(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 20.0 * 1000f64.powf(i as f64 / (n - 1).max(1) as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn peak_has_its_gain_at_centre_and_is_flat_far_away() {
        let b = peq_biquad(PeqType::Peak, 1000.0, 6.0, 1.4);
        assert!(close(b.magnitude_db(1000.0), 6.0, 0.01));
        assert!(close(b.magnitude_db(20.0), 0.0, 0.05));
        let cut = peq_biquad(PeqType::Peak, 1000.0, -6.0, 1.4);
        assert!(close(cut.magnitude_db(1000.0), -6.0, 0.01));
    }

    #[test]
    fn shelves_and_passes() {
        let ls = peq_biquad(PeqType::LowShelf, 200.0, 6.0, 0.71);
        assert!(close(ls.magnitude_db(20.0), 6.0, 0.2));
        assert!(close(ls.magnitude_db(10000.0), 0.0, 0.1));
        let hs = peq_biquad(PeqType::HighShelf, 2000.0, -4.0, 0.71);
        assert!(close(hs.magnitude_db(20000.0), -4.0, 0.3));
        let lp = peq_biquad(PeqType::LowPass12, 1000.0, 0.0, 0.707);
        assert!(close(lp.magnitude_db(1000.0), -3.0, 0.1));
        assert!(lp.magnitude_db(10000.0) < -35.0);
        let hp = peq_biquad(PeqType::HighPass6, 1000.0, 0.0, 0.707);
        assert!(close(hp.magnitude_db(1000.0), -3.0, 0.1));
        assert!(close(
            peq_biquad(PeqType::Allpass2, 1000.0, 0.0, 1.0).magnitude_db(300.0),
            0.0,
            1e-9
        ));
    }

    #[test]
    fn crossover_slopes() {
        // LK-24 is -6 dB at fc; BW-24 is -3 dB at fc.
        let lr = CutFilter {
            freq: 170,
            slope: 9,
        };
        assert!(close(cut_db(&lr, true, 1000.0), -6.02, 0.01));
        let bw = CutFilter {
            freq: 170,
            slope: 7,
        };
        assert!(close(cut_db(&bw, false, 1000.0), -3.01, 0.01));
        // 24 dB/octave one octave away.
        assert!(close(cut_db(&bw, true, 500.0), -24.1, 0.3));
        // Defaults (19.7 Hz HP / 20.16 kHz LP, 6 dB/oct) barely touch the audio band.
        let hp = CutFilter { freq: 0, slope: 0 };
        assert!(cut_db(&hp, true, 1000.0) > -0.01);
    }

    #[test]
    fn factory_band_is_flat() {
        let band = PeqBand {
            gain: 120,
            freq: 170,
            q: 35,
            kind: 0,
        };
        let b = band_response(&band).unwrap();
        assert!(freq_axis(50)
            .iter()
            .all(|f| b.magnitude_db(*f).abs() < 1e-9));
        assert!(band_response(&PeqBand { kind: 9, ..band }).is_none());
    }

    #[test]
    fn axis_spans_audio_band() {
        let a = freq_axis(301);
        assert!(close(a[0], 20.0, 1e-9) && close(a[300], 20000.0, 1e-6));
    }
}

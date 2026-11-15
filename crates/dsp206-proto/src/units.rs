//! Raw device values ↔ engineering units (memory-map.md §4, protocol.md §4.6).

/// Highest frequency index (memory-map.md §4.1).
pub const FREQ_INDEX_MAX: u16 = 300;
/// Index of 1 kHz.
pub const FREQ_INDEX_1K: u16 = 170;
/// Channel gain: raw 0..=400, 280 = 0 dB (memory-map.md §4.2).
pub const GAIN_MAX: u16 = 400;
pub const GAIN_UNITY: u16 = 280;
/// Mix gain is capped at 0 dB.
pub const MIX_GAIN_MAX: u16 = GAIN_UNITY;
/// PEQ / GEQ gain: raw 0..=240, 120 = 0 dB.
pub const EQ_GAIN_MAX: u16 = 240;
pub const EQ_GAIN_FLAT: u16 = 120;
/// Thresholds: `(raw - 180) / 2` dB. Gate tops out at 0 dB, compressor/limiter at +20 dB.
pub const GATE_THRESHOLD_MAX: u16 = 180;
pub const DYN_THRESHOLD_MAX: u16 = 220;
/// Delay in samples at 96 kHz (memory-map.md §4.7).
pub const DELAY_MAX: u16 = 65280;
pub const SAMPLE_RATE_KHZ: f64 = 96.0;
/// Speed of sound and feet conversion used by the original app.
pub const SPEED_OF_SOUND_M_PER_MS: f64 = 0.3435;
pub const FEET_PER_METER: f64 = 3.2808;
pub const KNEE_MAX: u16 = 12;
/// Attack 0..=998, hold 9..=998, release 9..=2999 (ms = raw + 1).
pub const ATTACK_MAX: u16 = 998;
pub const HOLD_MIN: u16 = 9;
pub const HOLD_MAX: u16 = 998;
pub const RELEASE_MIN: u16 = 9;
pub const RELEASE_MAX: u16 = 2999;

/// Q values by index (memory-map.md §4.4).
pub const Q_TABLE: [f64; 101] = [
    0.4, 0.42, 0.45, 0.47, 0.5, 0.53, 0.56, 0.59, 0.63, 0.67, 0.71, 0.75, 0.79, 0.84, 0.89, 0.94,
    1.0, 1.06, 1.12, 1.19, 1.26, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.4, 2.5, 2.7,
    2.8, 3.0, 3.2, 3.4, 3.6, 3.8, 4.0, 4.2, 4.5, 4.8, 5.0, 5.3, 5.7, 6.0, 6.3, 6.7, 7.1, 7.6, 8.0,
    8.5, 9.0, 9.5, 10.1, 10.7, 11.3, 12.0, 12.7, 13.5, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0,
    21.0, 23.0, 24.0, 25.0, 27.0, 29.0, 30.0, 32.0, 34.0, 36.0, 38.0, 40.0, 43.0, 45.0, 48.0, 51.0,
    54.0, 57.0, 60.0, 64.0, 68.0, 72.0, 76.0, 81.0, 85.0, 91.0, 96.0, 102.0, 108.0, 114.0, 121.0,
    128.0,
];

/// Compressor ratio by index (memory-map.md §4.8). The last entry means ∞.
pub const RATIO_TABLE: [f64; 16] = [
    1.0, 1.1, 1.3, 1.5, 1.7, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0, 6.0, 8.0, 10.0, 20.0, 20.1,
];

/// GEQ band centres in Hz (memory-map.md §4.3).
pub const GEQ_BANDS_HZ: [f64; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
    8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
];

/// PEQ filter types (memory-map.md §4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeqType {
    Peak = 0,
    LowShelf = 1,
    HighShelf = 2,
    LowPass6 = 3,
    LowPass12 = 4,
    HighPass6 = 5,
    HighPass12 = 6,
    Allpass1 = 7,
    Allpass2 = 8,
}

impl PeqType {
    pub const ALL: [PeqType; 9] = [
        PeqType::Peak,
        PeqType::LowShelf,
        PeqType::HighShelf,
        PeqType::LowPass6,
        PeqType::LowPass12,
        PeqType::HighPass6,
        PeqType::HighPass12,
        PeqType::Allpass1,
        PeqType::Allpass2,
    ];

    pub fn from_raw(raw: u8) -> Option<Self> {
        Self::ALL.get(raw as usize).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            PeqType::Peak => "Peak",
            PeqType::LowShelf => "Low Shelf",
            PeqType::HighShelf => "High Shelf",
            PeqType::LowPass6 => "LP -6dB",
            PeqType::LowPass12 => "LP -12dB",
            PeqType::HighPass6 => "HP -6dB",
            PeqType::HighPass12 => "HP -12dB",
            PeqType::Allpass1 => "Allpass1",
            PeqType::Allpass2 => "Allpass2",
        }
    }
}

/// Crossover filter family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterFamily {
    Butterworth,
    Bessel,
    LinkwitzRiley,
}

/// HPF/LPF slopes by raw index: (family, dB/octave) (memory-map.md §4.6).
pub const SLOPES: [(FilterFamily, u8); 20] = {
    use FilterFamily::{Bessel as BL, Butterworth as BW, LinkwitzRiley as LK};
    [
        (BW, 6),
        (BL, 6),
        (BW, 12),
        (BL, 12),
        (LK, 12),
        (BW, 18),
        (BL, 18),
        (BW, 24),
        (BL, 24),
        (LK, 24),
        (BW, 30),
        (BL, 30),
        (BW, 36),
        (BL, 36),
        (LK, 36),
        (BW, 42),
        (BL, 42),
        (BW, 48),
        (BL, 48),
        (LK, 48),
    ]
};

/// Label used by the original app, e.g. `"LK -24"`.
pub fn slope_label(raw: u8) -> Option<String> {
    let (family, db) = SLOPES.get(raw as usize)?;
    let tag = match family {
        FilterFamily::Butterworth => "BW",
        FilterFamily::Bessel => "BL",
        FilterFamily::LinkwitzRiley => "LK",
    };
    Some(format!("{tag} -{db}"))
}

fn clamp_round(x: f64, max: u16) -> u16 {
    x.round().clamp(0.0, f64::from(max)) as u16
}

/// Frequency index → Hz: `1000 · 2^((i − 170) / 30)`.
pub fn freq_hz(index: u16) -> f64 {
    1000.0 * 2f64.powf((f64::from(index) - f64::from(FREQ_INDEX_1K)) / 30.0)
}

/// Hz → nearest frequency index, clamped to 0..=300.
pub fn freq_index(hz: f64) -> u16 {
    if hz <= 0.0 {
        return 0;
    }
    clamp_round(
        30.0 * (hz / 1000.0).log2() + f64::from(FREQ_INDEX_1K),
        FREQ_INDEX_MAX,
    )
}

/// Channel or mix gain raw → dB.
pub fn gain_db(raw: u16) -> f64 {
    if raw >= 80 {
        (f64::from(raw) - f64::from(GAIN_UNITY)) / 10.0
    } else {
        -60.0 + f64::from(raw) / 2.0
    }
}

/// dB → channel gain raw, clamped to 0..=400.
pub fn gain_raw(db: f64) -> u16 {
    if db >= -20.0 {
        clamp_round(db * 10.0 + f64::from(GAIN_UNITY), GAIN_MAX)
    } else {
        clamp_round((db + 60.0) * 2.0, GAIN_MAX)
    }
}

/// PEQ/GEQ gain raw → dB.
pub fn eq_gain_db(raw: u16) -> f64 {
    (f64::from(raw) - f64::from(EQ_GAIN_FLAT)) / 10.0
}

pub fn eq_gain_raw(db: f64) -> u16 {
    clamp_round(db * 10.0 + f64::from(EQ_GAIN_FLAT), EQ_GAIN_MAX)
}

/// Gate/compressor/limiter threshold raw → dB.
pub fn threshold_db(raw: u16) -> f64 {
    (f64::from(raw) - 180.0) / 2.0
}

/// dB → threshold raw; pass `GATE_THRESHOLD_MAX` or `DYN_THRESHOLD_MAX`.
pub fn threshold_raw(db: f64, max: u16) -> u16 {
    clamp_round(db * 2.0 + 180.0, max)
}

/// Attack/hold/release raw → milliseconds.
pub fn time_ms(raw: u16) -> u32 {
    u32::from(raw) + 1
}

/// Milliseconds → attack/hold/release raw, clamped to `min..=max` raw.
pub fn time_raw(ms: u32, min: u16, max: u16) -> u16 {
    let raw = ms.saturating_sub(1).min(u32::from(u16::MAX)) as u16;
    raw.clamp(min, max)
}

pub fn delay_ms(raw: u16) -> f64 {
    f64::from(raw) / SAMPLE_RATE_KHZ
}

pub fn delay_raw(ms: f64) -> u16 {
    clamp_round(ms * SAMPLE_RATE_KHZ, DELAY_MAX)
}

pub fn delay_meters(raw: u16) -> f64 {
    delay_ms(raw) * SPEED_OF_SOUND_M_PER_MS
}

pub fn delay_feet(raw: u16) -> f64 {
    delay_meters(raw) * FEET_PER_METER
}

pub fn q_value(index: u8) -> Option<f64> {
    Q_TABLE.get(index as usize).copied()
}

/// Nearest Q index for a Q value (by log distance).
pub fn q_index(q: f64) -> u8 {
    let target = q.max(f64::MIN_POSITIVE).ln();
    let mut best = 0;
    for (i, v) in Q_TABLE.iter().enumerate() {
        if (v.ln() - target).abs() < (Q_TABLE[best].ln() - target).abs() {
            best = i;
        }
    }
    best as u8
}

pub fn ratio(index: u16) -> Option<f64> {
    RATIO_TABLE.get(index as usize).copied()
}

/// One 3-byte meter group → linear amplitude (protocol.md §4.6).
pub fn meter_amplitude(group: [u8; 3]) -> f32 {
    f32::from_le_bytes([0, group[2], group[0], group[1]])
}

/// Floor used by the original app for silent channels.
pub const METER_FLOOR: f64 = 1e-7;

/// One 3-byte meter group → dBu (`20·log10(x) + 18`).
pub fn meter_dbu(group: [u8; 3]) -> f64 {
    let x = f64::from(meter_amplitude(group));
    20.0 * x.max(METER_FLOOR).log10() + 18.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn frequency_scale_matches_original_app() {
        // UI shows "19.7Hz" at index 0 and "20.16KHz" at index 300.
        assert!(close(freq_hz(0), 19.69, 0.01));
        assert_eq!(freq_hz(170), 1000.0);
        assert!(close(freq_hz(300), 20158.7, 0.1));
        // Factory input PEQ centres are within 2 % of 50, 100, 200, 500, 1k, 2k, 5k, 10k Hz.
        let idx = [41u16, 71, 101, 140, 170, 200, 240, 270];
        let hz = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
        for (i, f) in idx.iter().zip(hz) {
            assert!(
                close(freq_hz(*i) / f, 1.0, 0.02),
                "index {i} = {} Hz",
                freq_hz(*i)
            );
        }
    }

    #[test]
    fn frequency_index_round_trips() {
        for i in 0..=FREQ_INDEX_MAX {
            assert_eq!(freq_index(freq_hz(i)), i);
        }
        assert_eq!(freq_index(1.0), 0);
        assert_eq!(freq_index(0.0), 0);
        assert_eq!(freq_index(1e6), FREQ_INDEX_MAX);
    }

    #[test]
    fn gain_scale_matches_rs232_help() {
        assert_eq!(gain_db(0), -60.0);
        assert_eq!(gain_db(80), -20.0);
        assert_eq!(gain_db(280), 0.0);
        assert_eq!(gain_db(400), 12.0);
        assert_eq!(gain_db(79), -20.5);
        for raw in 0..=GAIN_MAX {
            assert_eq!(gain_raw(gain_db(raw)), raw);
        }
        assert_eq!(gain_raw(99.0), GAIN_MAX);
        assert_eq!(gain_raw(-99.0), 0);
    }

    #[test]
    fn eq_gain_and_thresholds() {
        assert_eq!(eq_gain_db(120), 0.0);
        assert_eq!(eq_gain_db(0), -12.0);
        assert_eq!(eq_gain_db(240), 12.0);
        for raw in 0..=EQ_GAIN_MAX {
            assert_eq!(eq_gain_raw(eq_gain_db(raw)), raw);
        }
        assert_eq!(threshold_db(0), -90.0);
        assert_eq!(threshold_db(GATE_THRESHOLD_MAX), 0.0);
        assert_eq!(threshold_db(DYN_THRESHOLD_MAX), 20.0);
        assert_eq!(threshold_raw(20.0, GATE_THRESHOLD_MAX), GATE_THRESHOLD_MAX);
        for raw in 0..=DYN_THRESHOLD_MAX {
            assert_eq!(threshold_raw(threshold_db(raw), DYN_THRESHOLD_MAX), raw);
        }
    }

    #[test]
    fn times_and_delay() {
        // Factory defaults: attack 49 → 50 ms, release 499 → 500 ms.
        assert_eq!(time_ms(49), 50);
        assert_eq!(time_ms(499), 500);
        assert_eq!(time_raw(500, RELEASE_MIN, RELEASE_MAX), 499);
        assert_eq!(time_raw(1, RELEASE_MIN, RELEASE_MAX), RELEASE_MIN);
        assert_eq!(time_raw(99_999, 0, ATTACK_MAX), ATTACK_MAX);
        assert_eq!(delay_ms(96), 1.0);
        assert_eq!(delay_ms(DELAY_MAX), 680.0);
        assert_eq!(delay_raw(680.0), DELAY_MAX);
        assert_eq!(delay_raw(1000.0), DELAY_MAX);
        assert!(close(delay_meters(96), 0.3435, 1e-9));
        assert!(close(delay_feet(96), 0.3435 * 3.2808, 1e-9));
    }

    #[test]
    fn lookup_tables() {
        assert_eq!(q_value(0), Some(0.4));
        assert_eq!(q_value(16), Some(1.0));
        assert_eq!(q_value(35), Some(3.0)); // factory PEQ Q index
        assert_eq!(q_value(100), Some(128.0));
        assert_eq!(q_value(101), None);
        assert!(Q_TABLE.windows(2).all(|w| w[0] < w[1]));
        for i in 0..=100u8 {
            assert_eq!(q_index(Q_TABLE[i as usize]), i);
        }
        assert_eq!(ratio(0), Some(1.0));
        assert_eq!(ratio(9), Some(4.0));
        assert_eq!(ratio(16), None);
        assert_eq!(GEQ_BANDS_HZ[17], 1000.0);
        assert_eq!(slope_label(0).as_deref(), Some("BW -6"));
        assert_eq!(slope_label(9).as_deref(), Some("LK -24"));
        assert_eq!(slope_label(19).as_deref(), Some("LK -48"));
        assert_eq!(slope_label(20), None);
        assert_eq!(PeqType::from_raw(2), Some(PeqType::HighShelf));
        assert_eq!(PeqType::from_raw(9), None);
        assert_eq!(PeqType::Allpass2.label(), "Allpass2");
    }

    #[test]
    fn meter_decoding_matches_capture() {
        // Idle InA group from a captured 0x40 reply: about -87.75 dBu.
        assert!(close(meter_dbu([0xad, 0x36, 0x14]), -87.75, 0.01));
        // Silence is floored, never -inf.
        assert!(close(meter_dbu([0, 0, 0]), -122.0, 1e-9));
        // 1.0 linear (0x3f800000) is +18 dBu.
        assert!(close(meter_dbu([0x80, 0x3f, 0x00]), 18.0, 1e-9));
    }
}

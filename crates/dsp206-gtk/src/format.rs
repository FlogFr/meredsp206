//! Display and parsing of raw device values (memory-map.md §4).

use dsp206_link::UsbDevice;
use dsp206_proto::units;

pub fn hz(f: f64) -> String {
    if f >= 1000.0 {
        format!("{:.2} kHz", f / 1000.0)
    } else if f >= 100.0 {
        format!("{f:.0} Hz")
    } else {
        format!("{f:.1} Hz")
    }
}

pub fn freq_index(i: u16) -> String {
    hz(units::freq_hz(i))
}

pub fn db(v: f64) -> String {
    if v.abs() < 0.05 {
        "0.0 dB".into()
    } else {
        format!("{v:+.1} dB")
    }
}

pub fn gain(raw: u16) -> String {
    db(units::gain_db(raw))
}

pub fn eq_gain(raw: u16) -> String {
    db(units::eq_gain_db(raw))
}

pub fn threshold(raw: u16) -> String {
    db(units::threshold_db(raw))
}

pub fn q(index: u8) -> String {
    match units::q_value(index) {
        Some(v) if v >= 10.0 => format!("{v:.0}"),
        Some(v) => format!("{v:.2}"),
        None => "?".into(),
    }
}

pub fn ratio(index: u16) -> String {
    match units::ratio(index) {
        Some(_) if index == 15 => "∞:1".into(),
        Some(v) => format!("{v:.1}:1"),
        None => "?".into(),
    }
}

pub fn ms(v: u32) -> String {
    format!("{v} ms")
}

/// Delay in the unit chosen on the device (0 = ms, 1 = m, 2 = ft).
pub fn delay(raw: u16, unit: u16) -> String {
    match unit {
        1 => format!("{:.3} m", units::delay_meters(raw)),
        2 => format!("{:.2} ft", units::delay_feet(raw)),
        _ => format!("{:.2} ms", units::delay_ms(raw)),
    }
}

/// Parse a number with an optional unit suffix and `k` multiplier
/// ("1k", "1.5 kHz", "-3 dB", "+2,5").
/// Subtitle of the USB row on the device page.
pub fn usb_devices(found: &[UsbDevice]) -> String {
    match found {
        [] => "No unit plugged in".into(),
        [d] => format!("{} on {}", d.name, d.path.display()),
        [d, ..] => format!(
            "{} units plugged in, using {}",
            found.len(),
            d.path.display()
        ),
    }
}

pub fn parse_number(text: &str) -> Option<f64> {
    let t = text
        .trim()
        .to_ascii_lowercase()
        .replace(',', ".")
        .replace('−', "-");
    let t = t
        .trim_end_matches("hz")
        .trim_end_matches("db")
        .trim_end_matches("ms")
        .trim();
    let (num, mult) = match t.strip_suffix('k') {
        Some(n) => (n.trim(), 1000.0),
        None => (t, 1.0),
    };
    num.parse::<f64>().ok().map(|v| v * mult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(freq_index(0), "19.7 Hz");
        assert_eq!(freq_index(170), "1.00 kHz");
        assert_eq!(freq_index(300), "20.16 kHz");
        assert_eq!(freq_index(101), "203 Hz");
        assert_eq!(gain(280), "0.0 dB");
        assert_eq!(gain(400), "+12.0 dB");
        assert_eq!(gain(0), "-60.0 dB");
        assert_eq!(eq_gain(95), "-2.5 dB");
        assert_eq!(threshold(220), "+20.0 dB");
        assert_eq!(q(35), "3.00");
        assert_eq!(q(100), "128");
        assert_eq!(ratio(9), "4.0:1");
        assert_eq!(ratio(15), "∞:1");
        assert_eq!(delay(96, 0), "1.00 ms");
        assert_eq!(delay(960, 1), "3.435 m");
    }

    #[test]
    fn describes_usb_devices() {
        let dev = |n: u8| UsbDevice {
            path: format!("/dev/hidraw{n}").into(),
            name: "Musicrown Dsp Process".into(),
            phys: String::new(),
        };
        assert_eq!(usb_devices(&[]), "No unit plugged in");
        assert_eq!(
            usb_devices(&[dev(5)]),
            "Musicrown Dsp Process on /dev/hidraw5"
        );
        assert_eq!(
            usb_devices(&[dev(5), dev(6)]),
            "2 units plugged in, using /dev/hidraw5"
        );
    }

    #[test]
    fn parses_user_input() {
        assert_eq!(parse_number("1k"), Some(1000.0));
        assert_eq!(parse_number("1.5 kHz"), Some(1500.0));
        assert_eq!(parse_number(" -3 dB"), Some(-3.0));
        assert_eq!(parse_number("+2,5"), Some(2.5));
        assert_eq!(parse_number("−6"), Some(-6.0));
        assert_eq!(parse_number("12 ms"), Some(12.0));
        assert_eq!(parse_number("abc"), None);
    }
}

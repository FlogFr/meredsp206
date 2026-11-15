//! USB HID transport (protocol.md §1.2).
//!
//! The unit enumerates as a generic HID device with 64-byte input and output
//! reports and no report IDs. Each report carries one ordinary frame
//! (`frame` module) from its first byte; the rest of the report is padding.

use std::fmt;

use crate::frame::{MIN_FRAME, OVERHEAD};

/// USB vendor ID ("Musicrown").
pub const VENDOR_ID: u16 = 0x0168;
/// USB product ID ("Dsp Process").
pub const PRODUCT_ID: u16 = 0x0821;
/// Size of every input and output report.
pub const REPORT_LEN: usize = 64;
/// Report ID to put before an output report: the device does not number
/// its reports, which Linux `hidraw` and Windows both write as a leading 0.
pub const REPORT_ID: u8 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HidError {
    /// The frame does not fit in one report.
    FrameTooLong(usize),
}

impl fmt::Display for HidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HidError::FrameTooLong(n) => {
                write!(f, "frame of {n} bytes exceeds the {REPORT_LEN}-byte report")
            }
        }
    }
}

impl std::error::Error for HidError {}

/// The output report carrying `frame`: the frame bytes, then zero padding.
pub fn output_report(frame: &[u8]) -> Result<[u8; REPORT_LEN], HidError> {
    if frame.len() > REPORT_LEN {
        return Err(HidError::FrameTooLong(frame.len()));
    }
    let mut report = [0u8; REPORT_LEN];
    report[..frame.len()].copy_from_slice(frame);
    Ok(report)
}

/// The frame bytes inside an input report, without the padding.
///
/// Like the original editor, this trusts the length byte of the frame at
/// the start of the report (`report[4] + 8` bytes) and leaves validation to
/// the frame decoder. A report too short to hold a header is returned whole.
pub fn input_frame(report: &[u8]) -> &[u8] {
    if report.len() < MIN_FRAME {
        return report;
    }
    let len = report[4] as usize + OVERHEAD;
    &report[..len.min(report.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame, FrameDecoder};

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn output_report_is_frame_then_zero_padding() {
        let frame = hex("10 02 00 01 01 10 10 03 11"); // connect (protocol.md §2.1)
        let report = output_report(&frame).unwrap();
        assert_eq!(report.len(), 64);
        assert_eq!(&report[..9], &frame[..]);
        assert!(report[9..].iter().all(|&b| b == 0));
    }

    #[test]
    fn output_report_rejects_frames_longer_than_a_report() {
        let fits = Frame::request(1, 0x25, vec![0; REPORT_LEN - OVERHEAD - 1]);
        let bytes = fits.encode().unwrap();
        assert_eq!(bytes.len(), REPORT_LEN);
        assert!(output_report(&bytes).is_ok());
        let long = Frame::request(1, 0x25, vec![0; REPORT_LEN - OVERHEAD])
            .encode()
            .unwrap();
        assert_eq!(output_report(&long), Err(HidError::FrameTooLong(65)));
    }

    #[test]
    fn input_frame_cuts_the_padding_by_length() {
        // Identify reply (protocol.md §2.1), padded with stale bytes that look
        // like the start of another frame.
        let reply = hex("10 02 01 00 0e 13 44 53 50 32 30 36 20 56 30 31 30 35 50 10 03 4c");
        let mut report = reply.clone();
        report.extend([0x10, 0x02, 0x01, 0x00, 0x34]);
        report.resize(REPORT_LEN, 0xaa);
        assert_eq!(input_frame(&report), &reply[..]);
        let mut dec = FrameDecoder::new();
        dec.push(input_frame(&report));
        assert_eq!(dec.next().unwrap().unwrap().data, b"DSP206 V0105P");
        assert_eq!(dec.pending(), 0);
    }

    #[test]
    fn input_frame_never_reads_past_the_report() {
        let mut report = [0u8; REPORT_LEN];
        report[..5].copy_from_slice(&[0x10, 0x02, 0x01, 0x00, 0xff]);
        assert_eq!(input_frame(&report).len(), REPORT_LEN);
        assert_eq!(input_frame(&[0x10, 0x02]), &[0x10, 0x02]);
    }
}

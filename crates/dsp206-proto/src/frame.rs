//! Frame encoding and streaming decoding (protocol.md §2).
//!
//! ```text
//! 10 02 | src | dst | len | cmd | data[len-1] | 10 03 | xor
//! ```
//!
//! `len` counts the command byte, there is no DLE stuffing, and `xor` covers
//! every preceding byte of the frame.

use std::fmt;

pub const DLE: u8 = 0x10;
pub const STX: u8 = 0x02;
pub const ETX: u8 = 0x03;
/// Address of the PC (controller) side.
pub const PC_ADDRESS: u8 = 0x00;
/// Factory default device ID.
pub const DEFAULT_DEVICE_ID: u8 = 1;
/// Bytes around the payload: header (6) + trailer (3), minus the cmd byte counted in `len`.
pub const OVERHEAD: usize = 8;
/// Largest payload a frame can carry (`len` is one byte and counts `cmd`).
pub const MAX_DATA: usize = 254;
/// Smallest valid frame (no payload).
pub const MIN_FRAME: usize = OVERHEAD + 1;

/// One protocol frame, independent of direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub src: u8,
    pub dst: u8,
    pub cmd: u8,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// The payload does not fit in the one-byte length field.
    PayloadTooLong(usize),
    /// The bytes at `-3..-2` are not `10 03`.
    BadTrailer,
    /// The XOR checksum does not match.
    BadChecksum { expected: u8, actual: u8 },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::PayloadTooLong(n) => write!(f, "payload of {n} bytes exceeds {MAX_DATA}"),
            FrameError::BadTrailer => write!(f, "frame does not end with DLE ETX"),
            FrameError::BadChecksum { expected, actual } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:#04x}, got {actual:#04x}"
                )
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// XOR of all bytes.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |acc, b| acc ^ b)
}

impl Frame {
    /// A frame sent by the PC to device `dev`.
    pub fn request(dev: u8, cmd: u8, data: Vec<u8>) -> Self {
        Frame {
            src: PC_ADDRESS,
            dst: dev,
            cmd,
            data,
        }
    }

    /// Wire bytes of this frame.
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        if self.data.len() > MAX_DATA {
            return Err(FrameError::PayloadTooLong(self.data.len()));
        }
        let mut out = Vec::with_capacity(self.data.len() + MIN_FRAME);
        out.extend_from_slice(&[
            DLE,
            STX,
            self.src,
            self.dst,
            self.data.len() as u8 + 1,
            self.cmd,
        ]);
        out.extend_from_slice(&self.data);
        out.extend_from_slice(&[DLE, ETX]);
        out.push(checksum(&out));
        Ok(out)
    }

    /// Parse one complete frame (exactly `len + 8` bytes).
    pub fn decode(bytes: &[u8]) -> Result<Frame, FrameError> {
        let n = bytes.len();
        if n < MIN_FRAME || bytes[0] != DLE || bytes[1] != STX || bytes[4] as usize + OVERHEAD != n
        {
            return Err(FrameError::BadTrailer);
        }
        if bytes[n - 3] != DLE || bytes[n - 2] != ETX {
            return Err(FrameError::BadTrailer);
        }
        let expected = checksum(&bytes[..n - 1]);
        if expected != bytes[n - 1] {
            return Err(FrameError::BadChecksum {
                expected,
                actual: bytes[n - 1],
            });
        }
        Ok(Frame {
            src: bytes[2],
            dst: bytes[3],
            cmd: bytes[5],
            data: bytes[6..n - 3].to_vec(),
        })
    }
}

/// Incremental decoder for a byte stream (TCP or serial).
///
/// Bytes before a `10 02` header are skipped. A frame that fails validation is
/// reported once, then the decoder resynchronises on the next header.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Bytes buffered but not yet consumed.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    fn sync(&mut self) {
        match self.buf.windows(2).position(|w| w == [DLE, STX]) {
            Some(0) => {}
            Some(i) => {
                self.buf.drain(..i);
            }
            None => {
                // Keep a trailing DLE: it may start the next header.
                let keep = usize::from(self.buf.last() == Some(&DLE));
                let cut = self.buf.len() - keep;
                self.buf.drain(..cut);
            }
        }
    }
}

impl Iterator for FrameDecoder {
    type Item = Result<Frame, FrameError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.sync();
        if self.buf.len() < MIN_FRAME || !self.buf.starts_with(&[DLE, STX]) {
            return None;
        }
        let total = self.buf[4] as usize + OVERHEAD;
        if self.buf.len() < total {
            return None;
        }
        match Frame::decode(&self.buf[..total]) {
            Ok(frame) => {
                self.buf.drain(..total);
                Some(Ok(frame))
            }
            Err(e) => {
                // Drop this header and look for the next one.
                self.buf.drain(..2);
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    // Frames captured on a real DSP 206 (protocol.md §2.1).
    const CONNECT_TX: &str = "10 02 00 01 01 10 10 03 11";
    const CONNECT_RX: &str = "10 02 01 00 02 10 19 10 03 0b";
    const IDENTIFY_TX: &str = "10 02 00 01 01 13 10 03 12";
    const IDENTIFY_RX: &str = "10 02 01 00 0e 13 44 53 50 32 30 36 20 56 30 31 30 35 50 10 03 4c";
    const PAGE0_TX: &str = "10 02 00 01 02 27 00 10 03 25";

    #[test]
    fn encodes_captured_requests() {
        assert_eq!(
            Frame::request(1, 0x10, vec![]).encode().unwrap(),
            hex(CONNECT_TX)
        );
        assert_eq!(
            Frame::request(1, 0x13, vec![]).encode().unwrap(),
            hex(IDENTIFY_TX)
        );
        assert_eq!(
            Frame::request(1, 0x27, vec![0]).encode().unwrap(),
            hex(PAGE0_TX)
        );
    }

    #[test]
    fn decodes_captured_replies() {
        let f = Frame::decode(&hex(CONNECT_RX)).unwrap();
        assert_eq!((f.src, f.dst, f.cmd, f.data), (1, 0, 0x10, vec![0x19]));
        let f = Frame::decode(&hex(IDENTIFY_RX)).unwrap();
        assert_eq!(f.cmd, 0x13);
        assert_eq!(f.data, b"DSP206 V0105P");
    }

    #[test]
    fn round_trips_payload_containing_dle() {
        // No DLE stuffing: 0x10 bytes in the payload are sent as is.
        let f = Frame::request(1, 0x33, vec![0x10, 0x02, 0x10, 0x03, 0x10]);
        let bytes = f.encode().unwrap();
        assert_eq!(bytes.len(), 5 + MIN_FRAME);
        assert_eq!(Frame::decode(&bytes).unwrap(), f);
    }

    #[test]
    fn rejects_bad_checksum_and_trailer() {
        let mut bytes = hex(CONNECT_RX);
        bytes[6] ^= 0xff;
        assert!(matches!(
            Frame::decode(&bytes),
            Err(FrameError::BadChecksum { .. })
        ));
        let mut bytes = hex(CONNECT_RX);
        bytes[7] = 0x11;
        assert_eq!(Frame::decode(&bytes), Err(FrameError::BadTrailer));
    }

    #[test]
    fn rejects_oversized_payload() {
        let f = Frame::request(1, 0x26, vec![0; MAX_DATA + 1]);
        assert_eq!(f.encode(), Err(FrameError::PayloadTooLong(MAX_DATA + 1)));
        assert!(Frame::request(1, 0x26, vec![0; MAX_DATA]).encode().is_ok());
    }

    #[test]
    fn decoder_handles_split_and_concatenated_frames() {
        let mut stream = hex(CONNECT_RX);
        stream.extend(hex(IDENTIFY_RX));
        let mut dec = FrameDecoder::new();
        // Feed one byte at a time.
        let mut got = Vec::new();
        for b in &stream {
            dec.push(&[*b]);
            got.extend(dec.by_ref());
        }
        let cmds: Vec<u8> = got.into_iter().map(|r| r.unwrap().cmd).collect();
        assert_eq!(cmds, vec![0x10, 0x13]);
        assert_eq!(dec.pending(), 0);
    }

    #[test]
    fn decoder_skips_garbage_and_resyncs_after_corruption() {
        let mut stream = vec![0xff, 0x00, 0x10, 0x55];
        let mut bad = hex(CONNECT_RX);
        bad[6] ^= 1;
        stream.extend(&bad);
        stream.extend(hex(IDENTIFY_RX));
        let mut dec = FrameDecoder::new();
        dec.push(&stream);
        let results: Vec<_> = dec.by_ref().collect();
        assert!(matches!(results[0], Err(FrameError::BadChecksum { .. })));
        assert_eq!(results.last().unwrap().as_ref().unwrap().cmd, 0x13);
    }

    #[test]
    fn decoder_waits_for_incomplete_frame() {
        let full = hex(IDENTIFY_RX);
        let mut dec = FrameDecoder::new();
        dec.push(&full[..10]);
        assert!(dec.next().is_none());
        dec.push(&full[10..]);
        assert_eq!(dec.next().unwrap().unwrap().cmd, 0x13);
    }
}

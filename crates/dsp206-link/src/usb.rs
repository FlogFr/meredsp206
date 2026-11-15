//! USB HID link (protocol.md §1.2): finding the unit under
//! `/sys/class/hidraw` and exchanging 64-byte reports through `/dev/hidrawN`.
//!
//! Only `std` is used: the node is opened non-blocking and polled, like the
//! TCP socket with its short read timeout.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dsp206_proto::hid::{self, PRODUCT_ID, REPORT_ID, REPORT_LEN, VENDOR_ID};

use crate::transport::Transport;

/// Where the kernel lists `hidraw` nodes.
pub const SYS_HIDRAW: &str = "/sys/class/hidraw";
/// Bus type of USB in `HID_ID` (linux/input.h `BUS_USB`).
const BUS_USB: u16 = 0x0003;
/// Sleep after an empty read, so the worker does not spin.
const POLL: Duration = Duration::from_millis(5);

/// `O_NONBLOCK` from the Linux headers (asm-generic/fcntl.h, overridden by
/// MIPS and SPARC), so `std` alone can open the node with non-blocking reads.
#[cfg(any(target_arch = "mips", target_arch = "mips64"))]
const O_NONBLOCK: i32 = 0o200;
#[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
const O_NONBLOCK: i32 = 0x4000;
#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "sparc",
    target_arch = "sparc64"
)))]
const O_NONBLOCK: i32 = 0o4000;

/// A DSP 206 attached over USB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbDevice {
    /// Device node, e.g. `/dev/hidraw5`.
    pub path: PathBuf,
    /// Name given by the device, `Musicrown Dsp Process`.
    pub name: String,
    /// Where it is plugged, e.g. `usb-0000:02:00.0-6/input0`.
    pub phys: String,
}

/// The DSP described by the `device/uevent` file of a `hidraw` node, if it is
/// one (`HID_ID=0003:00000168:00000821`).
pub fn parse_uevent(node: &Path, text: &str) -> Option<UsbDevice> {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(str::to_string)
    };
    let id = field("HID_ID")?;
    let mut parts = id.split(':').map(|p| u32::from_str_radix(p, 16).ok());
    let (bus, vendor, product) = (parts.next()??, parts.next()??, parts.next()??);
    if (bus, vendor, product)
        != (
            u32::from(BUS_USB),
            u32::from(VENDOR_ID),
            u32::from(PRODUCT_ID),
        )
    {
        return None;
    }
    Some(UsbDevice {
        path: node.to_path_buf(),
        name: field("HID_NAME").unwrap_or_default(),
        phys: field("HID_PHYS").unwrap_or_default(),
    })
}

/// Every DSP 206 plugged in over USB. Reads sysfs only, sends nothing.
pub fn find_devices() -> io::Result<Vec<UsbDevice>> {
    find_in(Path::new(SYS_HIDRAW), Path::new("/dev"))
}

fn find_in(sys: &Path, dev: &Path) -> io::Result<Vec<UsbDevice>> {
    let entries = match fs::read_dir(sys) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut found: Vec<UsbDevice> = entries
        .flatten()
        .filter_map(|entry| {
            let text = fs::read_to_string(entry.path().join("device/uevent")).ok()?;
            parse_uevent(&dev.join(entry.file_name()), &text)
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// Something that moves whole reports, like a `hidraw` node: one write sends
/// one output report, one read returns one input report.
pub(crate) trait ReportPipe: Send {
    fn write_report(&mut self, report: &[u8]) -> io::Result<usize>;
    /// `WouldBlock` when no report is waiting.
    fn read_report(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

impl ReportPipe for File {
    fn write_report(&mut self, report: &[u8]) -> io::Result<usize> {
        self.write(report)
    }

    fn read_report(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read(buf)
    }
}

/// One frame per report, in both directions.
pub(crate) struct Hid<P> {
    pipe: P,
}

impl<P: ReportPipe> Hid<P> {
    pub(crate) fn new(pipe: P) -> Self {
        Hid { pipe }
    }
}

impl<P: ReportPipe> Transport for Hid<P> {
    fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        let report =
            hid::output_report(frame).map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))?;
        // hidraw expects the report ID first, 0 for unnumbered reports.
        let mut out = [REPORT_ID; 1 + REPORT_LEN];
        out[1..].copy_from_slice(&report);
        let n = self.pipe.write_report(&out).map_err(unplugged)?;
        if n != out.len() {
            return Err(io::Error::new(
                ErrorKind::WriteZero,
                format!("short USB write ({n} of {} bytes)", out.len()),
            ));
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut report = [0u8; REPORT_LEN];
        match self.pipe.read_report(&mut report) {
            Ok(n) => {
                let frame = hid::input_frame(&report[..n]);
                let len = frame.len().min(buf.len());
                buf[..len].copy_from_slice(&frame[..len]);
                if len == 0 {
                    // An empty report is not the end of the device.
                    return Err(ErrorKind::WouldBlock.into());
                }
                Ok(len)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(POLL);
                Err(e)
            }
            Err(e) => Err(unplugged(e)),
        }
    }

    fn close(&mut self) {}
}

fn unplugged(e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{e} (USB cable unplugged?)"))
}

/// Open a `hidraw` node for the link.
pub(crate) fn open(path: &Path) -> io::Result<Hid<File>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(path)
        .map_err(|e| {
            let hint = if e.kind() == ErrorKind::PermissionDenied {
                " (install data/udev/70-meredsp206.rules, see README)"
            } else {
                ""
            };
            io::Error::new(e.kind(), format!("{}: {e}{hint}", path.display()))
        })?;
    Ok(Hid::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::os::unix::net::UnixDatagram;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    use dsp206_proto::{Frame, FrameDecoder, Program, Request};

    use crate::{Event, Link, LinkConfig, MockDevice, MockOptions};

    // `device/uevent` of the unit plugged into the dev machine, and of a mouse.
    const DSP_UEVENT: &str = "DRIVER=hid-generic\nHID_ID=0003:00000168:00000821\n\
        HID_NAME=Musicrown Dsp Process\nHID_PHYS=usb-0000:02:00.0-6/input0\nHID_UNIQ=\n\
        MODALIAS=hid:b0003g0001v00000168p00000821\n";
    const MOUSE_UEVENT: &str = "DRIVER=hid-generic\nHID_ID=0003:00001BCF:00000005\n\
        HID_NAME=USB Optical Mouse\nHID_PHYS=usb-0000:2d:00.3-1.1.2/input0\n";

    impl ReportPipe for UnixDatagram {
        fn write_report(&mut self, report: &[u8]) -> io::Result<usize> {
            self.send(report)
        }

        fn read_report(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.recv(buf)
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dsp206-usb-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn recognises_the_unit_by_vendor_and_product() {
        let node = Path::new("/dev/hidraw5");
        assert_eq!(
            parse_uevent(node, DSP_UEVENT),
            Some(UsbDevice {
                path: node.into(),
                name: "Musicrown Dsp Process".into(),
                phys: "usb-0000:02:00.0-6/input0".into(),
            })
        );
        assert_eq!(parse_uevent(node, MOUSE_UEVENT), None);
        assert_eq!(parse_uevent(node, "HID_ID=0005:00000168:00000821\n"), None); // Bluetooth
        assert_eq!(parse_uevent(node, "HID_ID=garbage\n"), None);
        assert_eq!(parse_uevent(node, ""), None);
    }

    #[test]
    fn udev_rule_matches_the_ids() {
        let rule = include_str!("../../../data/udev/70-meredsp206.rules");
        assert!(rule.contains(&format!(r#"ATTRS{{idVendor}}=="{VENDOR_ID:04x}""#)));
        assert!(rule.contains(&format!(r#"ATTRS{{idProduct}}=="{PRODUCT_ID:04x}""#)));
        assert!(rule.contains(r#"SUBSYSTEM=="hidraw""#));
    }

    #[test]
    fn finds_devices_in_sysfs() {
        let sys = scratch_dir("sys");
        for (node, text) in [("hidraw0", MOUSE_UEVENT), ("hidraw5", DSP_UEVENT)] {
            fs::create_dir_all(sys.join(node).join("device")).unwrap();
            fs::write(sys.join(node).join("device/uevent"), text).unwrap();
        }
        fs::create_dir_all(sys.join("hidraw9")).unwrap(); // no uevent: skipped
        let found = find_in(&sys, Path::new("/dev")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, Path::new("/dev/hidraw5"));
        assert!(find_in(&sys.join("missing"), Path::new("/dev"))
            .unwrap()
            .is_empty());
        fs::remove_dir_all(&sys).unwrap();
    }

    #[test]
    fn sends_one_zero_padded_report_with_report_id() {
        let (ours, theirs) = UnixDatagram::pair().unwrap();
        ours.set_nonblocking(true).unwrap();
        let mut link = Hid::new(ours);
        let connect = Request::Connect.frame(1).encode().unwrap();
        link.send(&connect).unwrap();
        let mut got = [0xffu8; 100];
        let n = theirs.recv(&mut got).unwrap();
        assert_eq!(n, 65);
        assert_eq!(got[0], 0, "report ID");
        assert_eq!(&got[1..10], &connect[..]);
        assert!(got[10..65].iter().all(|&b| b == 0));
        assert!(link.send(&[0u8; 65]).is_err());
    }

    #[test]
    fn receives_the_frame_without_padding() {
        let (ours, theirs) = UnixDatagram::pair().unwrap();
        ours.set_nonblocking(true).unwrap();
        let mut link = Hid::new(ours);
        let mut buf = [0u8; 256];
        let err = link.recv(&mut buf).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::WouldBlock);
        let reply = Frame {
            src: 1,
            dst: 0,
            cmd: 0x10,
            data: vec![0x19],
        }
        .encode()
        .unwrap();
        let mut report = [0x10u8; REPORT_LEN]; // padding full of DLE bytes
        report[..reply.len()].copy_from_slice(&reply);
        theirs.send(&report).unwrap();
        let n = link.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], &reply[..]);
    }

    /// Plays the USB side of a unit by forwarding reports to the TCP mock.
    fn usb_bridge(
        usb: UnixDatagram,
        mock: std::net::SocketAddr,
        stop: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut tcp = TcpStream::connect(mock).unwrap();
            tcp.set_read_timeout(Some(Duration::from_millis(2)))
                .unwrap();
            usb.set_read_timeout(Some(Duration::from_millis(2)))
                .unwrap();
            let mut decoder = FrameDecoder::new();
            let (mut out, mut buf) = ([0u8; 128], [0u8; 512]);
            while !stop.load(Ordering::Relaxed) {
                if let Ok(n) = usb.recv(&mut out) {
                    assert_eq!((n, out[0]), (1 + REPORT_LEN, REPORT_ID));
                    tcp.write_all(hid::input_frame(&out[1..n])).unwrap();
                }
                if let Ok(n) = tcp.read(&mut buf) {
                    decoder.push(&buf[..n]);
                }
                for frame in decoder.by_ref().flatten() {
                    let mut report = hid::output_report(&frame.encode().unwrap()).unwrap();
                    let used = frame.data.len() + dsp206_proto::frame::MIN_FRAME;
                    report[used..].fill(0x5a);
                    usb.send(&report).unwrap();
                }
            }
        })
    }

    #[test]
    fn reads_everything_over_usb_reports() {
        let mock = MockDevice::start(MockOptions::default()).unwrap();
        let (ours, theirs) = UnixDatagram::pair().unwrap();
        ours.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let bridge = usb_bridge(theirs, mock.addr(), Arc::clone(&stop));
        let cfg = LinkConfig {
            read_timeout: Duration::from_millis(300),
            ..LinkConfig::default()
        };
        let link = Link::spawn(Box::new(Hid::new(ours)), cfg).unwrap();
        link.read_all();
        let end = Instant::now() + Duration::from_secs(10);
        let mut events = Vec::new();
        while Instant::now() < end {
            if let Some(ev) = link.recv_timeout(Duration::from_millis(10)) {
                let done = matches!(ev, Event::PresetSlots(_));
                events.push(ev);
                if done {
                    break;
                }
            }
        }
        link.close();
        let end = Instant::now() + Duration::from_secs(2);
        while mock.state().received.last() != Some(&Request::Disconnect) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, Ordering::Relaxed);
        bridge.join().unwrap();
        assert!(events.contains(&Event::Identity("DSP206 V0105P".into())));
        assert!(events.contains(&Event::Program(Program::factory_default())));
        assert!(!events.iter().any(|e| matches!(e, Event::Timeout(_))));
        let received = mock.state().received.clone();
        assert_eq!(received.first(), Some(&Request::Connect));
        assert_eq!(received.last(), Some(&Request::Disconnect));
    }

    #[test]
    fn open_errors_name_the_node_and_the_udev_rule() {
        let err = open(Path::new("/nonexistent/hidraw99")).err().unwrap();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(err.to_string().contains("/nonexistent/hidraw99"));
        // A node owned by root, as before the udev rule is installed.
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("perm");
        fs::create_dir_all(&dir).unwrap();
        let node = dir.join("hidraw0");
        fs::write(&node, b"").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o000)).unwrap();
        if let Err(err) = open(&node) {
            // (root can open anything, so only checked when it fails)
            assert_eq!(err.kind(), ErrorKind::PermissionDenied);
            assert!(err.to_string().contains("udev"), "{err}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}

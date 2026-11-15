//! Talking to a DSP 206 over TCP or USB (claude/docs/connection.md).
//!
//! A [`Link`] owns the single connection and a worker thread that keeps
//! exactly one request in flight, applies timeouts and retries, coalesces
//! queued setters and turns replies into [`Event`]s. No async runtime.

pub mod discovery;
pub mod mock;
mod transport;
pub mod usb;
mod worker;

use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use dsp206_proto::{Meters, Program, Request};

pub use discovery::{discover, Discovered};
pub use mock::{MockDevice, MockOptions, MockState};
pub use usb::{find_devices as find_usb_devices, UsbDevice};

use transport::Transport;

/// TCP port of the Ethernet module (Microchip UART-to-TCP bridge).
pub const PORT: u16 = 9761;

#[derive(Clone, Debug)]
pub struct LinkConfig {
    pub device_id: u8,
    pub connect_timeout: Duration,
    /// Reply timeout for reads (original app: 800 ms).
    pub read_timeout: Duration,
    /// How long to wait for the `0x01` acknowledgement of a setter.
    pub write_timeout: Duration,
    /// Acknowledgement wait for a preset store `0x21` (original app: 10 s).
    pub store_timeout: Duration,
    /// Acknowledgement wait for a preset recall `0x20` (original app: 50 s).
    pub recall_timeout: Duration,
    /// How many times an unanswered request is sent again (original app: 2).
    pub retries: u8,
    /// Quiet time after each reply or abandoned request before the next frame.
    pub gap: Duration,
    /// Level-meter poll period when metering is on (original app: 300 ms).
    pub meter_interval: Duration,
    /// Consecutive unanswered requests before the link is reported silent.
    pub silent_after: u32,
}

impl Default for LinkConfig {
    fn default() -> Self {
        LinkConfig {
            device_id: dsp206_proto::frame::DEFAULT_DEVICE_ID,
            connect_timeout: Duration::from_secs(3),
            read_timeout: Duration::from_millis(800),
            write_timeout: Duration::from_millis(500),
            store_timeout: Duration::from_secs(10),
            recall_timeout: Duration::from_secs(50),
            retries: 2,
            gap: Duration::ZERO,
            meter_interval: Duration::from_millis(300),
            silent_after: 3,
        }
    }
}

/// Whether the device is answering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Answering,
    /// Several requests in a row got no reply (connection.md §1).
    Silent,
}

/// What the worker reports. `Program` is large but rare, so it is not boxed.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Handshake(u8),
    Identity(String),
    CurrentPreset(u8),
    PresetSlots(Vec<u8>),
    PresetName {
        index: u8,
        name: String,
    },
    LockState(Vec<u8>),
    /// One program page arrived (`data` excludes the page number).
    Page {
        page: u8,
        data: Vec<u8>,
    },
    /// All 19 pages of a program read arrived.
    Program(Program),
    /// A program read finished with these pages missing.
    ProgramIncomplete(Vec<u8>),
    Meters(Meters),
    /// A setter was sent but no acknowledgement came back.
    Unconfirmed(Request),
    /// A read got no reply after all retries.
    Timeout(Request),
    Health(Health),
    /// Every frame sent (`outgoing`) or received, for the protocol log.
    Trace {
        outgoing: bool,
        bytes: Vec<u8>,
    },
    /// The connection ended; the worker has stopped.
    Closed(String),
}

pub(crate) enum Cmd {
    Send(Request),
    ReadAll,
    ReadProgram { fresh: bool },
    Metering(bool),
    Close,
}

/// The order used to read a device: the program first, because the device
/// may stop answering after a couple of seconds (connection.md §1).
pub fn read_all_sequence() -> Vec<Request> {
    let mut seq = vec![Request::Connect, Request::Identify];
    seq.extend(program_read_sequence());
    seq.push(Request::GetCurrentPreset);
    seq.extend((1..=20).map(Request::GetPresetName));
    seq.push(Request::GetLockState);
    seq.push(Request::GetPresetSlots);
    seq
}

/// Store the current program into user preset `n` (1 = U01) under `name`,
/// as the original editor does: `0x26` name, `0x21 [n]`, then `0x12`
/// (protocol.md §4.2). The worker sends each step after the previous ACK.
pub fn store_preset_sequence(n: u8, name: &str) -> Vec<Request> {
    vec![
        Request::SetPresetName(dsp206_proto::program::preset_name_field(name)),
        Request::StorePreset(n),
        Request::EndOfTransfer,
    ]
}

/// Page reads, then `0x12` as the original editor does after a bulk read.
pub fn program_read_sequence() -> Vec<Request> {
    let mut seq: Vec<Request> = (0..dsp206_proto::program::PAGE_COUNT as u8)
        .map(Request::ReadProgramPage)
        .collect();
    seq.push(Request::EndOfTransfer);
    seq
}

/// A connection to one device.
pub struct Link {
    tx: Sender<Cmd>,
    rx: Receiver<Event>,
    pending: Arc<AtomicUsize>,
    handle: Option<JoinHandle<()>>,
}

impl Link {
    /// Connect to the Ethernet module.
    pub fn connect(addr: SocketAddr, config: LinkConfig) -> io::Result<Link> {
        let stream = TcpStream::connect_timeout(&addr, config.connect_timeout)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_millis(5)))?;
        Link::spawn(Box::new(stream), config)
    }

    /// Open a unit plugged in over USB, by its `hidraw` node (see
    /// [`find_usb_devices`]).
    pub fn open_usb(path: &Path, config: LinkConfig) -> io::Result<Link> {
        Link::spawn(Box::new(usb::open(path)?), config)
    }

    pub(crate) fn spawn(io: Box<dyn Transport>, config: LinkConfig) -> io::Result<Link> {
        let (tx, cmd_rx) = mpsc::channel();
        let (ev_tx, rx) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending);
        let handle = std::thread::Builder::new()
            .name("dsp206-link".into())
            .spawn(move || worker::run(io, config, cmd_rx, ev_tx, worker_pending))?;
        Ok(Link {
            tx,
            rx,
            pending,
            handle: Some(handle),
        })
    }

    /// Queue a request. Setters with the same target replace queued ones.
    pub fn send(&self, req: Request) {
        let _ = self.tx.send(Cmd::Send(req));
    }

    /// Queue the full read: handshake, identity, program, presets, lock state.
    pub fn read_all(&self) {
        let _ = self.tx.send(Cmd::ReadAll);
    }

    /// Queue a full program read (after a recall, for example).
    pub fn read_program(&self) {
        let _ = self.tx.send(Cmd::ReadProgram { fresh: true });
    }

    /// Ask again only for the pages a previous read did not get.
    pub fn retry_missing_pages(&self) {
        let _ = self.tx.send(Cmd::ReadProgram { fresh: false });
    }

    pub fn set_metering(&self, on: bool) {
        let _ = self.tx.send(Cmd::Metering(on));
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<Event> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Requests queued or in flight.
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }

    /// Send `0x11` (best effort) and stop the worker.
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Cmd::Close);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.shutdown();
    }
}

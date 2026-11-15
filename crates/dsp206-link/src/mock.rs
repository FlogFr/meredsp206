//! A simulated DSP 206 speaking the real protocol over TCP on localhost.
//!
//! Used by the tests (never the real device, see device-safety.md) and by the
//! GUI's demo mode. Replies follow what the real unit sent in our captures;
//! setters are applied to an in-memory [`Program`] and acknowledged with
//! `0x01`.

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dsp206_proto::program::{name_field, PAGE_COUNT, PROGRAM_NAME_LEN};
use dsp206_proto::{Frame, FrameDecoder, Program, Request};

#[derive(Clone, Debug, Default)]
pub struct MockOptions {
    /// Stop answering after this many replies, like the real unit
    /// (connection.md §1). `None` = always answer.
    pub reply_limit: Option<usize>,
    /// Do not acknowledge setters.
    pub silent_writes: bool,
}

/// Everything the simulated device remembers.
#[derive(Clone, Debug)]
pub struct MockState {
    pub program: Program,
    /// 21 preset slots (name index 0..=20 as in `0x29`).
    pub presets: Vec<Program>,
    pub current_preset: u8,
    pub pending_name: Option<[u8; PROGRAM_NAME_LEN]>,
    pub tone: (u8, u8),
    /// Every request received, in order.
    pub received: Vec<Request>,
    pub replies_sent: usize,
}

impl Default for MockState {
    fn default() -> Self {
        let program = Program::factory_default();
        MockState {
            presets: vec![program.clone(); 21],
            program,
            current_preset: 0,
            pending_name: None,
            tone: (0, 0),
            received: Vec::new(),
            replies_sent: 0,
        }
    }
}

pub struct MockDevice {
    addr: SocketAddr,
    state: Arc<Mutex<MockState>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockDevice {
    /// Start on an ephemeral localhost port.
    pub fn start(options: MockOptions) -> std::io::Result<MockDevice> {
        Self::start_on(
            "127.0.0.1:0".parse().map_err(std::io::Error::other)?,
            options,
        )
    }

    pub fn start_on(addr: SocketAddr, options: MockOptions) -> std::io::Result<MockDevice> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(MockState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st) = (Arc::clone(&state), Arc::clone(&stop));
        let handle = std::thread::Builder::new()
            .name("dsp206-mock".into())
            .spawn(move || serve(listener, s, st, options))?;
        Ok(MockDevice {
            addr,
            state,
            stop,
            handle: Some(handle),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Lock and inspect or change the simulated state.
    pub fn state(&self) -> MutexGuard<'_, MockState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for MockDevice {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn lock(state: &Mutex<MockState>) -> MutexGuard<'_, MockState> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

fn serve(
    listener: TcpListener,
    state: Arc<Mutex<MockState>>,
    stop: Arc<AtomicBool>,
    opts: MockOptions,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => client(stream, &state, &stop, &opts),
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(_) => return,
        }
    }
}

fn client(mut stream: TcpStream, state: &Mutex<MockState>, stop: &AtomicBool, opts: &MockOptions) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
    let _ = stream.set_nodelay(true);
    let started = Instant::now();
    let mut decoder = FrameDecoder::new();
    let mut buf = [0u8; 512];
    while !stop.load(Ordering::Relaxed) {
        match stream.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => decoder.push(&buf[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => continue,
            Err(_) => return,
        }
        for frame in decoder.by_ref().flatten() {
            let reply = {
                let mut st = lock(state);
                let Some(req) = Request::parse(frame.cmd, &frame.data) else {
                    continue;
                };
                st.received.push(req.clone());
                let limited = opts.reply_limit.is_some_and(|n| st.replies_sent >= n);
                let reply = respond(&mut st, &req, frame.dst, started.elapsed(), opts);
                if limited {
                    None
                } else {
                    if reply.is_some() {
                        st.replies_sent += 1;
                    }
                    reply
                }
            };
            if let Some(f) = reply {
                if let Ok(bytes) = f.encode() {
                    if stream.write_all(&bytes).is_err() {
                        return;
                    }
                }
            }
        }
    }
}

fn reply(dev: u8, cmd: u8, data: Vec<u8>) -> Option<Frame> {
    Some(Frame {
        src: dev,
        dst: 0,
        cmd,
        data,
    })
}

fn respond(
    st: &mut MockState,
    req: &Request,
    dev: u8,
    t: Duration,
    opts: &MockOptions,
) -> Option<Frame> {
    use Request::*;
    let ack = || {
        if opts.silent_writes {
            None
        } else {
            reply(dev, 0x01, vec![])
        }
    };
    match req {
        Connect => reply(dev, 0x10, vec![0x19]),
        Disconnect => None,
        Identify => reply(dev, 0x13, b"DSP206 V0105P".to_vec()),
        GetCurrentPreset => reply(dev, 0x14, vec![st.current_preset]),
        GetLockState => reply(dev, 0x2C, vec![0x00, 0x27, 0x0f, 0x00, 0x00, 0x00]),
        GetPresetSlots => reply(dev, 0x22, vec![0; 20]),
        GetPresetName(n) => {
            let p = st.presets.get(*n as usize)?;
            let mut d = vec![*n];
            d.extend_from_slice(&p.name);
            reply(dev, 0x29, d)
        }
        ReadProgramPage(p) if (*p as usize) < PAGE_COUNT => {
            let pages = st.program.to_pages();
            let mut d = vec![*p];
            d.extend_from_slice(&pages[*p as usize]);
            reply(dev, 0x24, d)
        }
        GetMeters => reply(dev, 0x40, meters(&st.program, t)),
        EndOfTransfer => ack(),
        SetPresetName(name) => {
            st.pending_name = Some(*name);
            ack()
        }
        StorePreset(n) => {
            let mut p = st.program.clone();
            if let Some(name) = st.pending_name.take() {
                p.name = name;
            }
            *st.presets.get_mut(*n as usize)? = p;
            ack()
        }
        RecallPreset(n) => {
            st.program = st.presets.get(*n as usize)?.clone();
            st.current_preset = *n;
            ack()
        }
        TestTone { source, freq } => {
            st.tone = (*source, *freq);
            ack()
        }
        other => {
            st.program.apply(other);
            ack()
        }
    }
}

/// Synthetic level meters: each unmuted channel moves slowly around a level
/// set by its gain.
fn meters(p: &Program, t: Duration) -> Vec<u8> {
    let mut d = Vec::with_capacity(38);
    let groups: [Option<dsp206_proto::Channel>; 10] = [
        dsp206_proto::Channel::new(0),
        dsp206_proto::Channel::new(1),
        None,
        None,
        dsp206_proto::Channel::new(2),
        dsp206_proto::Channel::new(3),
        dsp206_proto::Channel::new(4),
        dsp206_proto::Channel::new(5),
        dsp206_proto::Channel::new(6),
        dsp206_proto::Channel::new(7),
    ];
    for (i, ch) in groups.iter().enumerate() {
        let amp = match ch {
            Some(c) if !p.is_muted(*c) => {
                let gain_db = dsp206_proto::units::gain_db(p.strip(*c).gain);
                let wobble = (t.as_secs_f64() * 1.3 + i as f64).sin() * 6.0;
                let dbu = -14.0 + gain_db + wobble;
                10f64.powf((dbu - 18.0) / 20.0) as f32
            }
            _ => 5e-6,
        };
        let b = amp.to_le_bytes();
        d.extend_from_slice(&[b[2], b[3], b[1]]);
    }
    d.extend_from_slice(&[0; 8]);
    d
}

/// Factory name helper for tests.
pub fn preset_name(text: &str) -> [u8; PROGRAM_NAME_LEN] {
    name_field(text)
}

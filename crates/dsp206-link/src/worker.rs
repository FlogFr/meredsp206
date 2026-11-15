//! The link worker thread: one request in flight, timeouts, retries,
//! coalescing (protocol.md §3.2) and reply decoding.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

use dsp206_proto::{FrameDecoder, ProgramAssembler, Reply, Request};

use crate::{program_read_sequence, read_all_sequence, Cmd, Event, Health, LinkConfig};

struct Inflight {
    req: Request,
    bytes: Vec<u8>,
    deadline: Instant,
    tries: u8,
}

struct Worker {
    stream: TcpStream,
    cfg: LinkConfig,
    events: Sender<Event>,
    pending: Arc<AtomicUsize>,
    queue: VecDeque<Request>,
    inflight: Option<Inflight>,
    decoder: FrameDecoder,
    assembler: ProgramAssembler,
    reading_program: bool,
    metering: bool,
    last_meter: Instant,
    misses: u32,
    health: Health,
}

pub(crate) fn run(
    stream: TcpStream,
    cfg: LinkConfig,
    cmds: Receiver<Cmd>,
    events: Sender<Event>,
    pending: Arc<AtomicUsize>,
) {
    let mut w = Worker {
        stream,
        cfg,
        events,
        pending,
        queue: VecDeque::new(),
        inflight: None,
        decoder: FrameDecoder::new(),
        assembler: ProgramAssembler::new(),
        reading_program: false,
        metering: false,
        last_meter: Instant::now(),
        misses: 0,
        health: Health::Answering,
    };
    let reason = w.run(&cmds);
    let _ = w.stream.shutdown(Shutdown::Both);
    let _ = w.events.send(Event::Closed(reason));
}

impl Worker {
    fn run(&mut self, cmds: &Receiver<Cmd>) -> String {
        loop {
            loop {
                match cmds.try_recv() {
                    Ok(Cmd::Send(req)) => self.enqueue(req),
                    Ok(Cmd::ReadAll) => {
                        self.start_program_read(true);
                        read_all_sequence()
                            .into_iter()
                            .for_each(|r| self.enqueue(r));
                    }
                    Ok(Cmd::ReadProgram { fresh }) => {
                        self.start_program_read(fresh);
                        program_read_sequence()
                            .into_iter()
                            .for_each(|r| self.enqueue(r));
                    }
                    Ok(Cmd::Metering(on)) => self.metering = on,
                    Ok(Cmd::Close) | Err(TryRecvError::Disconnected) => {
                        let _ = self.write(&frame_bytes(&Request::Disconnect, self.cfg.device_id));
                        return "closed".into();
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if let Err(e) = self.pump() {
                return e;
            }
            if let Err(e) = self.tick() {
                return e;
            }
            self.pending.store(
                self.queue.len() + usize::from(self.inflight.is_some()),
                Ordering::Relaxed,
            );
        }
    }

    fn emit(&self, ev: Event) {
        let _ = self.events.send(ev);
    }

    /// `fresh` restarts from scratch; otherwise pages already received are kept
    /// and only the missing ones are requested again.
    fn start_program_read(&mut self, fresh: bool) {
        if fresh || self.assembler.is_complete() {
            self.assembler = ProgramAssembler::new();
        }
        self.reading_program = true;
    }

    fn enqueue(&mut self, req: Request) {
        if let Request::ReadProgramPage(p) = req {
            if !self.assembler.missing_pages().contains(&p) {
                return;
            }
        }
        if let Some(key) = req.coalesce_key() {
            if let Some(slot) = self
                .queue
                .iter_mut()
                .find(|q| q.coalesce_key() == Some(key))
            {
                *slot = req;
                return;
            }
        } else if self.queue.contains(&req) {
            return;
        }
        self.queue.push_back(req);
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.emit(Event::Trace {
            outgoing: true,
            bytes: bytes.to_vec(),
        });
        self.stream
            .write_all(bytes)
            .map_err(|e| format!("send failed: {e}"))
    }

    /// Read whatever the socket has and handle complete frames.
    fn pump(&mut self) -> Result<(), String> {
        let mut buf = [0u8; 1024];
        match self.stream.read(&mut buf) {
            Ok(0) => return Err("the device closed the connection".into()),
            Ok(n) => self.decoder.push(&buf[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(format!("receive failed: {e}")),
        }
        while let Some(item) = self.decoder.next() {
            let Ok(frame) = item else { continue };
            self.emit(Event::Trace {
                outgoing: false,
                bytes: frame.encode().unwrap_or_default(),
            });
            if frame.src != self.cfg.device_id {
                continue;
            }
            self.answered();
            let reply = Reply::parse(&frame);
            let done = self
                .inflight
                .as_ref()
                .is_some_and(|inf| match inf.req.reply_code() {
                    Some(code) => code == frame.cmd && matches_index(&inf.req, &reply),
                    None => true,
                });
            // Store the reply (e.g. the last page) before finishing the request.
            self.handle_reply(reply);
            if done {
                let req = self.inflight.take().map(|i| i.req);
                self.after_request(req.as_ref());
            }
        }
        Ok(())
    }

    fn answered(&mut self) {
        self.misses = 0;
        if self.health != Health::Answering {
            self.health = Health::Answering;
            self.emit(Event::Health(Health::Answering));
        }
    }

    fn missed(&mut self) {
        self.misses += 1;
        if self.misses >= self.cfg.silent_after && self.health != Health::Silent {
            self.health = Health::Silent;
            self.emit(Event::Health(Health::Silent));
        }
    }

    fn handle_reply(&mut self, reply: Reply) {
        let ev = match reply {
            Reply::Connected { status } => Event::Handshake(status),
            Reply::Identity(s) => Event::Identity(s),
            Reply::CurrentPreset(n) => Event::CurrentPreset(n),
            Reply::PresetSlots(v) => Event::PresetSlots(v),
            Reply::PresetName { index, name } => Event::PresetName { index, name },
            Reply::LockState(v) => Event::LockState(v),
            Reply::Meters(m) => Event::Meters(m),
            Reply::ProgramPage { page, data } => {
                if self.assembler.insert(page, &data).is_err() {
                    return;
                }
                Event::Page { page, data }
            }
            Reply::Ack | Reply::Nak | Reply::Other(_) => return,
        };
        self.emit(ev);
    }

    /// Called when a request completes or is abandoned.
    fn after_request(&mut self, req: Option<&Request>) {
        if !matches!(req, Some(Request::ReadProgramPage(_))) || !self.reading_program {
            return;
        }
        let more = self
            .queue
            .iter()
            .any(|r| matches!(r, Request::ReadProgramPage(_)));
        if more {
            return;
        }
        self.reading_program = false;
        match self.assembler.program() {
            Some(p) => self.emit(Event::Program(p)),
            None => self.emit(Event::ProgramIncomplete(self.assembler.missing_pages())),
        }
    }

    fn tick(&mut self) -> Result<(), String> {
        let now = Instant::now();
        if let Some(inf) = &mut self.inflight {
            if now < inf.deadline {
                return Ok(());
            }
            let is_read = inf.req.reply_code().is_some();
            if is_read && inf.tries < self.cfg.read_retries && inf.req != Request::GetMeters {
                inf.tries += 1;
                inf.deadline = now + self.cfg.read_timeout;
                let bytes = inf.bytes.clone();
                return self.write(&bytes);
            }
            let Some(inf) = self.inflight.take() else {
                return Ok(());
            };
            self.missed();
            match inf.req {
                Request::GetMeters => {}
                ref r if is_read => self.emit(Event::Timeout(r.clone())),
                ref r if !matches!(r, Request::Disconnect | Request::EndOfTransfer) => {
                    self.emit(Event::Unconfirmed(r.clone()))
                }
                _ => {}
            }
            self.after_request(Some(&inf.req));
        }
        let next = match self.queue.pop_front() {
            Some(r) => r,
            None if self.metering
                && now.duration_since(self.last_meter) >= self.cfg.meter_interval =>
            {
                self.last_meter = now;
                Request::GetMeters
            }
            None => return Ok(()),
        };
        let bytes = frame_bytes(&next, self.cfg.device_id);
        let timeout = if next.reply_code().is_some() {
            self.cfg.read_timeout
        } else {
            self.cfg.write_timeout
        };
        self.write(&bytes)?;
        self.inflight = Some(Inflight {
            req: next,
            bytes,
            deadline: now + timeout,
            tries: 0,
        });
        Ok(())
    }
}

/// Replies that carry an index must match the request's index.
fn matches_index(req: &Request, reply: &Reply) -> bool {
    match (req, reply) {
        (Request::ReadProgramPage(p), Reply::ProgramPage { page, .. }) => p == page,
        (Request::GetPresetName(n), Reply::PresetName { index, .. }) => n == index,
        _ => true,
    }
}

pub(crate) fn frame_bytes(req: &Request, dev: u8) -> Vec<u8> {
    req.frame(dev).encode().unwrap_or_default()
}

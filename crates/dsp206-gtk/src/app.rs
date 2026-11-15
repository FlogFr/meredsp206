//! Application state and the glue between the model, the link and the UI.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use dsp206_link::{Event, Health, Link, LinkConfig, MockDevice, MockOptions};
use dsp206_proto::{Channel, Meters, Program, Request};
use gtk::{gio, glib};

use crate::model::Model;
use crate::ui::Ui;

pub const APP_ID: &str = "org.meredsp206.Editor";
/// The Ethernet module needs time before it serves a new session
/// (connection.md §2).
pub const RECONNECT_DELAY: Duration = Duration::from_secs(60);
const LOG_LINES: usize = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Offline,
    Connecting,
    Device,
    Demo,
}

pub struct App {
    pub model: RefCell<Model>,
    link: RefCell<Option<Link>>,
    demo: RefCell<Option<MockDevice>>,
    pub kind: Cell<LinkKind>,
    pub address: RefCell<String>,
    pub device_id: Cell<u8>,
    pub selected: Cell<Channel>,
    pub selected_band: Cell<usize>,
    pub meters: RefCell<Option<Meters>>,
    pub health: Cell<Health>,
    pub unconfirmed: Cell<u32>,
    pub timeouts: Cell<u32>,
    pub missing_pages: RefCell<Vec<u8>>,
    pub last_close: Cell<Option<Instant>>,
    pub log: RefCell<VecDeque<String>>,
    pub log_dirty: Cell<bool>,
    /// Set while widgets are being refreshed from the model, so their change
    /// signals do not send anything.
    pub updating: Cell<bool>,
    started: Instant,
    pub ui: OnceCell<Ui>,
}

/// Command-line options.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub demo: bool,
    pub connect: Option<String>,
    pub page: Option<String>,
    pub channel: Option<String>,
}

pub fn run(opts: Options) -> glib::ExitCode {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(move |application| {
        if let Some(win) = application.active_window() {
            win.present();
            return;
        }
        let app = App::new();
        let ui = crate::ui::build(application, &app);
        ui.window.present();
        let _ = app.ui.set(ui);
        app.refresh();
        let poll = Rc::clone(&app);
        glib::timeout_add_local(Duration::from_millis(30), move || {
            poll.poll();
            glib::ControlFlow::Continue
        });
        let tick = Rc::clone(&app);
        glib::timeout_add_local(Duration::from_millis(500), move || {
            tick.refresh_status();
            glib::ControlFlow::Continue
        });
        if let Some(ch) = opts.channel.as_deref().and_then(parse_channel) {
            app.select_channel(ch);
        }
        if let Some(page) = &opts.page {
            if let Some(ui) = app.ui() {
                ui.stack.set_visible_child_name(page);
            }
        }
        if opts.demo {
            app.start_demo();
        } else if let Some(ip) = &opts.connect {
            *app.address.borrow_mut() = ip.clone();
            app.connect_device();
        }
    });
    // Our options are parsed in main(); GTK only sees the program name.
    let argv0: Vec<String> = std::env::args().take(1).collect();
    application.run_with_args(&argv0)
}

impl App {
    fn new() -> Rc<App> {
        Rc::new(App {
            model: RefCell::new(Model::default()),
            link: RefCell::new(None),
            demo: RefCell::new(None),
            kind: Cell::new(LinkKind::Offline),
            address: RefCell::new("192.168.1.101".into()),
            device_id: Cell::new(1),
            selected: Cell::new(Channel::IN_A),
            selected_band: Cell::new(0),
            meters: RefCell::new(None),
            health: Cell::new(Health::Answering),
            unconfirmed: Cell::new(0),
            timeouts: Cell::new(0),
            missing_pages: RefCell::new(Vec::new()),
            last_close: Cell::new(None),
            log: RefCell::new(VecDeque::new()),
            log_dirty: Cell::new(false),
            updating: Cell::new(false),
            started: Instant::now(),
            ui: OnceCell::new(),
        })
    }

    pub fn ui(&self) -> Option<&Ui> {
        self.ui.get()
    }

    pub fn toast(&self, text: &str) {
        if let Some(ui) = self.ui() {
            ui.toasts.add_toast(adw::Toast::new(text));
        }
    }

    pub fn is_connected(&self) -> bool {
        self.link.borrow().is_some()
    }

    /// A user edit coming from a widget.
    pub fn edit(&self, req: Request) {
        if self.updating.get() {
            return;
        }
        let sent = self.model.borrow_mut().edit(req);
        if let Some(r) = sent {
            self.send(r);
            self.refresh();
        }
    }

    pub fn send(&self, req: Request) {
        if let Some(link) = self.link.borrow().as_ref() {
            link.send(req);
        }
    }

    pub fn send_all(&self, reqs: Vec<Request>) {
        for r in reqs {
            self.send(r);
        }
    }

    pub fn undo(&self) {
        let reqs = self.model.borrow_mut().undo();
        self.send_all(reqs);
        self.refresh();
    }

    pub fn redo(&self) {
        let reqs = self.model.borrow_mut().redo();
        self.send_all(reqs);
        self.refresh();
    }

    pub fn select_channel(&self, ch: Channel) {
        if self.selected.get() != ch {
            self.selected.set(ch);
            self.selected_band.set(0);
            self.refresh();
        }
    }

    // ---- connection -----------------------------------------------------

    /// Seconds to wait before reconnecting to a real unit is advisable.
    pub fn reconnect_wait(&self) -> Option<u64> {
        let since = self.last_close.get()?.elapsed();
        (since < RECONNECT_DELAY).then(|| (RECONNECT_DELAY - since).as_secs() + 1)
    }

    pub fn connect_device(self: &Rc<Self>) {
        let text = self.address.borrow().trim().to_string();
        let Ok(ip) = text.parse::<IpAddr>() else {
            self.toast(&format!("“{text}” is not an IP address"));
            return;
        };
        if let Some(s) = self.reconnect_wait() {
            self.toast(&format!(
                "The unit may stay silent for about {s} s after the last session; connecting anyway"
            ));
        }
        self.start_link(SocketAddr::new(ip, dsp206_link::PORT), LinkKind::Device);
    }

    pub fn start_demo(self: &Rc<Self>) {
        self.disconnect();
        match MockDevice::start(MockOptions::default()) {
            Ok(mock) => {
                let addr = mock.addr();
                *self.demo.borrow_mut() = Some(mock);
                self.start_link(addr, LinkKind::Demo);
            }
            Err(e) => self.toast(&format!("Cannot start the simulated device: {e}")),
        }
    }

    fn start_link(self: &Rc<Self>, addr: SocketAddr, kind: LinkKind) {
        if kind == LinkKind::Device {
            self.disconnect();
        }
        self.kind.set(LinkKind::Connecting);
        self.refresh_status();
        let cfg = LinkConfig {
            device_id: if kind == LinkKind::Demo {
                1
            } else {
                self.device_id.get()
            },
            ..LinkConfig::default()
        };
        let handle = gio::spawn_blocking(move || Link::connect(addr, cfg));
        let app = Rc::clone(self);
        glib::spawn_future_local(async move {
            match handle.await {
                Ok(Ok(link)) => {
                    link.read_all();
                    link.set_metering(true);
                    *app.link.borrow_mut() = Some(link);
                    app.kind.set(kind);
                    app.health.set(Health::Answering);
                    app.unconfirmed.set(0);
                    app.timeouts.set(0);
                    app.toast(&format!("Connected to {addr}, reading the program…"));
                }
                Ok(Err(e)) => {
                    app.kind.set(LinkKind::Offline);
                    app.toast(&format!("Cannot connect to {addr}: {e}"));
                }
                Err(_) => app.kind.set(LinkKind::Offline),
            }
            app.refresh();
        });
    }

    pub fn disconnect(&self) {
        let was_device = self.kind.get() == LinkKind::Device;
        if let Some(link) = self.link.borrow_mut().take() {
            link.close();
            if was_device {
                self.last_close.set(Some(Instant::now()));
            }
        }
        self.demo.borrow_mut().take();
        self.kind.set(LinkKind::Offline);
        *self.meters.borrow_mut() = None;
        self.refresh();
    }

    pub fn read_from_device(&self) {
        match self.link.borrow().as_ref() {
            Some(link) => link.read_all(),
            None => self.toast("Not connected"),
        }
    }

    pub fn retry_missing(&self) {
        match self.link.borrow().as_ref() {
            Some(link) => link.retry_missing_pages(),
            None => self.toast("Not connected"),
        }
    }

    pub fn push_program(&self) {
        let reqs = self.model.borrow().program.sync_requests();
        let n = reqs.len();
        if self.is_connected() {
            self.send_all(reqs);
            self.toast(&format!("Sending {n} settings to the device…"));
        } else {
            self.toast("Not connected");
        }
    }

    pub fn recall_preset(&self, n: u8) {
        let link = self.link.borrow();
        let Some(link) = link.as_ref() else {
            self.toast("Not connected");
            return;
        };
        // The worker keeps the order: recall first, then re-read the program.
        link.send(Request::RecallPreset(n));
        link.read_program();
        link.send(Request::GetCurrentPreset);
        self.toast(&format!("Recalling {}…", Model::preset_label(n)));
    }

    pub fn store_preset(&self, n: u8, name: &str) {
        if !self.is_connected() {
            self.toast("Not connected");
            return;
        }
        self.send(Request::SetPresetName(dsp206_proto::program::name_field(
            name,
        )));
        self.send(Request::StorePreset(n));
        self.send(Request::GetPresetName(n));
        self.toast(&format!("Storing {} “{name}”…", Model::preset_label(n)));
    }

    pub fn load_program(&self, p: Program) {
        let reqs = self.model.borrow_mut().load(p);
        self.send_all(reqs);
        self.refresh();
    }

    // ---- events ----------------------------------------------------------

    fn poll(&self) {
        let mut events = Vec::new();
        if let Some(link) = self.link.borrow().as_ref() {
            while let Some(ev) = link.try_recv() {
                events.push(ev);
                if events.len() > 500 {
                    break;
                }
            }
        }
        if events.is_empty() {
            return;
        }
        let mut program_changed = false;
        let mut meters = false;
        for ev in events {
            match ev {
                Event::Trace { outgoing, bytes } => self.trace(outgoing, &bytes),
                Event::Handshake(_) => {}
                Event::Identity(id) => self.model.borrow_mut().identity = Some(id),
                Event::CurrentPreset(n) => self.model.borrow_mut().current_preset = Some(n),
                Event::PresetName { index, name } => {
                    if let Some(slot) = self.model.borrow_mut().preset_names.get_mut(index as usize)
                    {
                        *slot = name;
                    }
                    program_changed = true;
                }
                Event::Page { page, data } => {
                    self.model.borrow_mut().apply_page(page, &data);
                    program_changed = true;
                }
                Event::Program(p) => {
                    self.model.borrow_mut().set_from_device(p);
                    self.missing_pages.borrow_mut().clear();
                    program_changed = true;
                    self.toast("Program read from the device");
                }
                Event::ProgramIncomplete(missing) => {
                    self.toast(&format!(
                        "{} of 19 program pages did not arrive. Use “Retry missing pages” later.",
                        missing.len()
                    ));
                    *self.missing_pages.borrow_mut() = missing;
                    program_changed = true;
                }
                Event::Meters(m) => {
                    *self.meters.borrow_mut() = Some(m);
                    meters = true;
                }
                Event::Unconfirmed(_) => self.unconfirmed.set(self.unconfirmed.get() + 1),
                Event::Timeout(_) => self.timeouts.set(self.timeouts.get() + 1),
                Event::Health(h) => {
                    self.health.set(h);
                    if h == Health::Silent {
                        self.toast("The device stopped answering. Changes are still sent but not confirmed.");
                    }
                }
                Event::LockState(_) | Event::PresetSlots(_) => {}
                Event::Closed(reason) => {
                    self.link.borrow_mut().take();
                    if self.kind.get() == LinkKind::Device {
                        self.last_close.set(Some(Instant::now()));
                    }
                    self.kind.set(LinkKind::Offline);
                    self.toast(&format!("Disconnected: {reason}"));
                    program_changed = true;
                }
            }
        }
        if program_changed {
            self.refresh();
        } else {
            self.refresh_status();
        }
        if meters {
            if let Some(ui) = self.ui() {
                ui.redraw_meters();
            }
        }
    }

    fn trace(&self, outgoing: bool, bytes: &[u8]) {
        let t = self.started.elapsed().as_secs_f64();
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let what = bytes
            .get(5)
            .map(|c| describe(*c, outgoing))
            .unwrap_or_default();
        let mut log = self.log.borrow_mut();
        log.push_back(format!(
            "{t:9.3}  {}  {:<44} {what}",
            if outgoing { "→" } else { "←" },
            hex.join(" ")
        ));
        while log.len() > LOG_LINES {
            log.pop_front();
        }
        self.log_dirty.set(true);
    }

    // ---- UI refresh --------------------------------------------------------

    pub fn refresh(&self) {
        if let Some(ui) = self.ui() {
            self.updating.set(true);
            ui.refresh(self);
            self.updating.set(false);
        }
    }

    pub fn refresh_status(&self) {
        if let Some(ui) = self.ui() {
            ui.refresh_status(self);
        }
    }
}

/// `ina`, `inb`, `out1`..`out6`.
pub fn parse_channel(s: &str) -> Option<Channel> {
    match s.to_ascii_lowercase().as_str() {
        "ina" => Some(Channel::IN_A),
        "inb" => Some(Channel::IN_B),
        o => o
            .strip_prefix("out")?
            .parse()
            .ok()
            .and_then(Channel::output),
    }
}

/// Short name of a command for the protocol log.
pub fn describe(cmd: u8, outgoing: bool) -> &'static str {
    match (cmd, outgoing) {
        (0x01, false) => "ack",
        (0x02, false) => "nak/busy",
        (0x10, _) => "connect",
        (0x11, _) => "disconnect",
        (0x12, _) => "end of transfer",
        (0x13, _) => "identify",
        (0x14, _) => "current preset",
        (0x15, _) => "delay unit",
        (0x20, _) => "store preset",
        (0x21, _) => "recall preset",
        (0x22, _) => "preset slots",
        (0x24, false) => "program page",
        (0x26, _) => "preset name",
        (0x27, _) => "read page",
        (0x29, _) => "preset name",
        (0x2A, _) => "copy channel",
        (0x2C, _) => "lock state",
        (0x30, _) => "compressor",
        (0x31, _) => "low-pass",
        (0x32, _) => "high-pass",
        (0x33, _) => "peq band",
        (0x34, _) => "gain",
        (0x35, _) => "mute",
        (0x36, _) => "polarity",
        (0x38, _) => "delay",
        (0x39, _) => "test tone",
        (0x3A, _) => "routing",
        (0x3B, _) => "link",
        (0x3C, _) => "peq bypass",
        (0x3D, _) => "channel name",
        (0x3E, _) => "gate",
        (0x3F, _) => "limiter",
        (0x40, _) => "meters",
        (0x41, _) => "mix gain",
        (0x48, _) => "geq band",
        (0x49, _) => "geq bypass",
        _ => "",
    }
}

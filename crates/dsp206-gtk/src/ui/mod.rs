//! Window, header bar, actions and the pages.

mod channel;
mod device;
mod draw;
mod log;
mod mixer;
mod widgets;

use std::rc::Rc;

use adw::prelude::*;
use dsp206_link::Health;
use dsp206_proto::Program;
use gtk::{gdk, gio, glib};

use crate::app::{App, LinkKind};

const CSS: &str = "
.mute-on { background: @error_bg_color; color: @error_fg_color; }
.toggle-on { background: @accent_bg_color; color: @accent_fg_color; }
.bypass-on { background: @warning_bg_color; color: @warning_fg_color; }
.strip { padding: 8px; border-radius: 12px; background: alpha(@card_bg_color, 1); border: 1px solid alpha(@borders, 1); }
.strip.selected { border-color: @accent_bg_color; }
.strip-name { font-weight: bold; }
.mono { font-family: monospace; }
.statusbar { padding: 4px 12px; font-size: 0.9em; }
.value { font-feature-settings: 'tnum'; }
";

pub struct Ui {
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub stack: adw::ViewStack,
    status: gtk::Label,
    link_button: gtk::Button,
    undo: gtk::Button,
    redo: gtk::Button,
    mixer: mixer::Mixer,
    channel: channel::ChannelPage,
    device: device::DevicePage,
    log: log::LogPage,
}

impl Ui {
    pub fn refresh(&self, app: &App) {
        self.mixer.refresh(app);
        self.channel.refresh(app);
        self.device.refresh(app);
        self.refresh_status(app);
    }

    pub fn refresh_status(&self, app: &App) {
        let m = app.model.borrow();
        self.undo.set_sensitive(m.can_undo());
        self.redo.set_sensitive(m.can_redo());
        let addr = app.address.borrow().clone();
        let (label, icon) = match app.kind.get() {
            LinkKind::Offline => ("Offline".to_string(), "network-offline-symbolic"),
            LinkKind::Connecting => (
                "Connecting…".to_string(),
                "network-transmit-receive-symbolic",
            ),
            LinkKind::Demo => (
                "Demo (simulated)".to_string(),
                "applications-science-symbolic",
            ),
            LinkKind::Device => (format!("DSP 206 · {addr}"), "network-wired-symbolic"),
        };
        self.link_button
            .set_child(Some(&widgets::icon_label(icon, &label)));
        let mut parts = Vec::new();
        match app.kind.get() {
            LinkKind::Offline => {
                parts.push("Not connected: edits stay local".to_string());
                if let Some(s) = app.reconnect_wait() {
                    parts.push(format!("unit ready again in ~{s} s"));
                }
            }
            LinkKind::Connecting => parts.push("Connecting…".into()),
            LinkKind::Device | LinkKind::Demo => {
                if let Some(id) = &m.identity {
                    parts.push(id.clone());
                }
                parts.push(match app.health.get() {
                    Health::Answering => "device answering".into(),
                    Health::Silent => "device silent: changes sent without confirmation".into(),
                });
                if let Some(n) = m.current_preset {
                    parts.push(format!("preset {}", crate::model::Model::preset_label(n)));
                }
            }
        }
        let missing = app.missing_pages.borrow();
        if !missing.is_empty() {
            parts.push(format!("{} program pages missing", missing.len()));
        }
        if app.unconfirmed.get() > 0 {
            parts.push(format!(
                "{} changes not acknowledged",
                app.unconfirmed.get()
            ));
        }
        if app.timeouts.get() > 0 {
            parts.push(format!("{} reads timed out", app.timeouts.get()));
        }
        self.status.set_text(&parts.join("  ·  "));
        if app.log_dirty.replace(false) {
            self.log.refresh(app);
        }
    }

    pub fn redraw_meters(&self) {
        self.mixer.redraw_meters();
        self.channel.redraw_meters();
    }
}

fn action(application: &adw::Application, name: &str, accels: &[&str], f: impl Fn() + 'static) {
    let a = gio::SimpleAction::new(name, None);
    a.connect_activate(move |_, _| f());
    application.add_action(&a);
    if !accels.is_empty() {
        application.set_accels_for_action(&format!("app.{name}"), accels);
    }
}

fn file_filter() -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("DSP 206 program (*.dsp206)"));
    filter.add_pattern("*.dsp206");
    let store = gio::ListStore::new::<gtk::FileFilter>();
    store.append(&filter);
    store
}

fn open_file(app: &Rc<App>, window: &adw::ApplicationWindow) {
    let dialog = gtk::FileDialog::builder()
        .title("Open program")
        .filters(&file_filter())
        .build();
    let app = Rc::clone(app);
    dialog.open(Some(window), gio::Cancellable::NONE, move |res| {
        let Ok(file) = res else { return };
        let Some(path) = file.path() else { return };
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| Program::from_file(&b).map_err(|e| e.to_string()))
        {
            Ok(p) => {
                app.load_program(p);
                app.toast(&format!("Opened {}", path.display()));
            }
            Err(e) => app.toast(&format!("{}: {e}", path.display())),
        }
    });
}

fn save_file(app: &Rc<App>, window: &adw::ApplicationWindow) {
    let name = format!(
        "{}.dsp206",
        app.model.borrow().program.name_text().replace('/', "-")
    );
    let dialog = gtk::FileDialog::builder()
        .title("Save program")
        .initial_name(name)
        .filters(&file_filter())
        .build();
    let app = Rc::clone(app);
    dialog.save(Some(window), gio::Cancellable::NONE, move |res| {
        let Ok(file) = res else { return };
        let Some(mut path) = file.path() else { return };
        if path.extension().is_none() {
            path.set_extension("dsp206");
        }
        let bytes = app.model.borrow().program.to_file();
        match std::fs::write(&path, bytes) {
            Ok(()) => app.toast(&format!("Saved {}", path.display())),
            Err(e) => app.toast(&format!("{}: {e}", path.display())),
        }
    });
}

pub fn build(application: &adw::Application, app: &Rc<App>) -> Ui {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let stack = adw::ViewStack::new();
    let mixer = mixer::Mixer::build(app);
    let channel = channel::ChannelPage::build(app);
    let device = device::DevicePage::build(app);
    let log = log::LogPage::build(app);
    stack.add_titled_with_icon(
        &mixer.root,
        Some("mixer"),
        "Mixer",
        "audio-volume-high-symbolic",
    );
    stack.add_titled_with_icon(
        &channel.root,
        Some("channel"),
        "Channel",
        "preferences-system-symbolic",
    );
    stack.add_titled_with_icon(
        &device.root,
        Some("device"),
        "Device",
        "audio-card-symbolic",
    );
    stack.add_titled_with_icon(&log.root, Some("log"), "Log", "utilities-terminal-symbolic");

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&stack));

    let header = adw::HeaderBar::new();
    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    header.set_title_widget(Some(&switcher));

    let link_button = gtk::Button::new();
    link_button.set_tooltip_text(Some("Connection settings"));
    {
        let stack = stack.clone();
        link_button.connect_clicked(move |_| stack.set_visible_child_name("device"));
    }
    let undo = gtk::Button::from_icon_name("edit-undo-symbolic");
    undo.set_tooltip_text(Some("Undo (Ctrl+Z)"));
    undo.set_action_name(Some("app.undo"));
    let redo = gtk::Button::from_icon_name("edit-redo-symbolic");
    redo.set_tooltip_text(Some("Redo (Ctrl+Shift+Z)"));
    redo.set_action_name(Some("app.redo"));
    header.pack_start(&link_button);
    header.pack_start(&undo);
    header.pack_start(&redo);

    let menu = gio::Menu::new();
    let file = gio::Menu::new();
    file.append(Some("Open Program…"), Some("app.open"));
    file.append(Some("Save Program As…"), Some("app.save"));
    menu.append_section(None, &file);
    let dev = gio::Menu::new();
    dev.append(Some("Read From Device"), Some("app.read"));
    dev.append(Some("Retry Missing Pages"), Some("app.retry"));
    dev.append(Some("Send Whole Program To Device"), Some("app.push"));
    dev.append(Some("Disconnect"), Some("app.disconnect"));
    menu.append_section(None, &dev);
    let help = gio::Menu::new();
    help.append(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
    help.append(Some("About meredsp206"), Some("app.about"));
    menu.append_section(None, &help);
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Main menu")
        .build();
    header.pack_end(&menu_button);

    let status = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["statusbar", "dim-label"])
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toasts));
    toolbar.add_bottom_bar(&status);

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("meredsp206")
        .default_width(1440)
        .default_height(900)
        .content(&toolbar)
        .build();

    let a = Rc::clone(app);
    action(application, "undo", &["<Control>z"], move || a.undo());
    let a = Rc::clone(app);
    action(
        application,
        "redo",
        &["<Control><Shift>z", "<Control>y"],
        move || a.redo(),
    );
    let (a, w) = (Rc::clone(app), window.clone());
    action(application, "open", &["<Control>o"], move || {
        open_file(&a, &w)
    });
    let (a, w) = (Rc::clone(app), window.clone());
    action(application, "save", &["<Control>s"], move || {
        save_file(&a, &w)
    });
    let a = Rc::clone(app);
    action(application, "read", &["<Control>r"], move || {
        a.read_from_device()
    });
    let a = Rc::clone(app);
    action(application, "retry", &[], move || a.retry_missing());
    let a = Rc::clone(app);
    action(application, "push", &[], move || a.push_program());
    let a = Rc::clone(app);
    action(application, "disconnect", &[], move || a.disconnect());
    let s = stack.clone();
    action(application, "page-mixer", &["<Alt>1"], move || {
        s.set_visible_child_name("mixer")
    });
    let s = stack.clone();
    action(application, "page-channel", &["<Alt>2"], move || {
        s.set_visible_child_name("channel")
    });
    let s = stack.clone();
    action(application, "page-device", &["<Alt>3"], move || {
        s.set_visible_child_name("device")
    });
    let s = stack.clone();
    action(application, "page-log", &["<Alt>4"], move || {
        s.set_visible_child_name("log")
    });
    let w = window.clone();
    action(application, "about", &[], move || {
        let about = adw::AboutDialog::builder()
            .application_name("meredsp206")
            .application_icon("audio-card")
            .version(env!("CARGO_PKG_VERSION"))
            .comments("Linux editor for the t.racks DSP 206 loudspeaker processor.\nNot affiliated with Thomann or the manufacturer.")
            .build();
        about.present(Some(&w));
    });
    let w = window.clone();
    action(
        application,
        "shortcuts",
        &["<Control>question"],
        move || {
            let dialog = adw::AlertDialog::new(Some("Keyboard Shortcuts"), Some(SHORTCUTS));
            dialog.add_response("close", "Close");
            dialog.present(Some(&w));
        },
    );
    let w = window.clone();
    action(application, "quit", &["<Control>q"], move || w.close());
    {
        let app = Rc::clone(app);
        window.connect_close_request(move |_| {
            app.disconnect();
            glib::Propagation::Proceed
        });
    }

    Ui {
        window,
        toasts,
        stack,
        status,
        link_button,
        undo,
        redo,
        mixer,
        channel,
        device,
        log,
    }
}

const SHORTCUTS: &str = "\
Ctrl+Z / Ctrl+Shift+Z: undo / redo
Ctrl+O / Ctrl+S: open / save a program file
Ctrl+R: read the program from the device
Alt+1…4: Mixer, Channel, Device, Log
EQ graph: drag a band handle to change frequency and gain, scroll to change Q
Ctrl+Q: quit";

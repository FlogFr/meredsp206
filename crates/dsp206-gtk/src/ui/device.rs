//! Device page: connection, discovery, presets, test tone, display unit.

use std::cell::RefCell;
use std::net::IpAddr;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use dsp206_proto::units::GEQ_BANDS_HZ;
use dsp206_proto::{Request, ToneSource};
use gtk::{gio, glib};

use super::widgets::{self, row};
use crate::app::{App, LinkKind};
use crate::format;
use crate::model::Model;

struct PresetRow {
    row: adw::ActionRow,
    store: gtk::Button,
}

pub struct DevicePage {
    pub root: adw::PreferencesPage,
    address: gtk::Entry,
    device_id: gtk::SpinButton,
    usb: adw::ActionRow,
    connect_usb: gtk::Button,
    connect: gtk::Button,
    demo: gtk::Button,
    disconnect: gtk::Button,
    info: [adw::ActionRow; 4],
    presets: Vec<PresetRow>,
    delay_unit: gtk::DropDown,
}

impl DevicePage {
    pub fn build(app: &Rc<App>) -> DevicePage {
        let root = adw::PreferencesPage::new();

        let conn = adw::PreferencesGroup::builder()
            .title("Connection")
            .description("Over USB, or over the network. The DSP accepts one network client at a time; after a network disconnection, wait about a minute before connecting again.")
            .build();
        let connect_usb = gtk::Button::with_label("Connect");
        connect_usb.set_valign(gtk::Align::Center);
        connect_usb.set_tooltip_text(Some("Connect to the unit plugged in over USB"));
        let usb = row("USB", None, &connect_usb);
        conn.add(&usb);
        {
            let app = Rc::clone(app);
            connect_usb.connect_clicked(move |_| app.connect_usb());
        }
        let address = gtk::Entry::builder()
            .text(app.address.borrow().as_str())
            .width_chars(16)
            .valign(gtk::Align::Center)
            .build();
        {
            let app = Rc::clone(app);
            address.connect_changed(move |e| *app.address.borrow_mut() = e.text().to_string());
        }
        conn.add(&row(
            "IP address",
            Some("factory default 192.168.1.101, TCP port 9761"),
            &address,
        ));
        let device_id = gtk::SpinButton::with_range(1.0, 254.0, 1.0);
        device_id.set_value(f64::from(app.device_id.get()));
        device_id.set_valign(gtk::Align::Center);
        {
            let app = Rc::clone(app);
            device_id.connect_value_changed(move |s| app.device_id.set(s.value() as u8));
        }
        conn.add(&row(
            "Device ID",
            Some("1 unless changed on the unit"),
            &device_id,
        ));
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_valign(gtk::Align::Center);
        let scan = gtk::Button::with_label("Scan network");
        scan.set_tooltip_text(Some("Find units on the network (UDP 30303)"));
        let connect = gtk::Button::with_label("Connect");
        connect.set_tooltip_text(Some("Connect over the network to the IP address above"));
        connect.add_css_class("suggested-action");
        let demo = gtk::Button::with_label("Demo");
        demo.set_tooltip_text(Some("Use a simulated DSP 206"));
        let disconnect = gtk::Button::with_label("Disconnect");
        for b in [&scan, &demo, &disconnect, &connect] {
            buttons.append(b);
        }
        conn.add(&row("", None, &buttons));
        root.add(&conn);
        {
            let app = Rc::clone(app);
            connect.connect_clicked(move |_| app.connect_device());
        }
        {
            let app = Rc::clone(app);
            demo.connect_clicked(move |_| app.start_demo());
        }
        {
            let app = Rc::clone(app);
            disconnect.connect_clicked(move |_| app.disconnect());
        }

        let found = adw::PreferencesGroup::builder()
            .title("Found on the network")
            .build();
        found.set_visible(false);
        root.add(&found);
        let found_rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::default();
        {
            let (app, found, address) = (Rc::clone(app), found.clone(), address.clone());
            scan.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                let extra: Vec<IpAddr> = app.address.borrow().parse().ok().into_iter().collect();
                let handle = gio::spawn_blocking(move || {
                    dsp206_link::discover(Duration::from_secs(2), &extra)
                });
                let (app, found, address, btn) =
                    (Rc::clone(&app), found.clone(), address.clone(), btn.clone());
                let rows = Rc::clone(&found_rows);
                glib::spawn_future_local(async move {
                    let res = handle.await;
                    btn.set_sensitive(true);
                    for r in rows.borrow_mut().drain(..) {
                        found.remove(&r);
                    }
                    match res {
                        Ok(Ok(list)) if !list.is_empty() => {
                            found.set_visible(true);
                            for d in list {
                                let use_btn = gtk::Button::with_label("Use");
                                use_btn.set_valign(gtk::Align::Center);
                                let r = row(
                                    &d.addr.to_string(),
                                    Some(&format!("{} {}", d.name, d.details.join(" "))),
                                    &use_btn,
                                );
                                let (address, ip) = (address.clone(), d.addr.to_string());
                                use_btn.connect_clicked(move |_| address.set_text(&ip));
                                found.add(&r);
                                rows.borrow_mut().push(r);
                            }
                        }
                        Ok(Ok(_)) => app.toast("No DSP 206 answered the discovery"),
                        Ok(Err(e)) => app.toast(&format!("Discovery failed: {e}")),
                        Err(_) => {}
                    }
                });
            });
        }

        let infog = adw::PreferencesGroup::builder().title("Device").build();
        let info = ["Model", "Link", "Current preset", "Program"].map(|t| {
            let r = adw::ActionRow::builder().title(t).build();
            r.add_css_class("property");
            infog.add(&r);
            r
        });
        root.add(&infog);

        let presetg = adw::PreferencesGroup::builder()
            .title("Presets")
            .description(
                "Recall replaces the current program on the unit; store overwrites a user preset.",
            )
            .build();
        let mut presets = Vec::new();
        for n in 0..21u8 {
            let r = adw::ActionRow::builder()
                .title(Model::preset_label(n))
                .build();
            let recall = gtk::Button::with_label("Recall");
            recall.set_valign(gtk::Align::Center);
            let store = gtk::Button::with_label("Store…");
            store.set_valign(gtk::Align::Center);
            store.set_visible(n > 0);
            r.add_suffix(&recall);
            r.add_suffix(&store);
            {
                let app = Rc::clone(app);
                recall.connect_clicked(move |_| app.recall_preset(n));
            }
            {
                let app = Rc::clone(app);
                store.connect_clicked(move |_| store_dialog(&app, n));
            }
            presetg.add(&r);
            presets.push(PresetRow { row: r, store });
        }
        root.add(&presetg);

        let tone = adw::PreferencesGroup::builder()
            .title("Test tone")
            .description("Replaces the input signal with a generator. Set it back to Analog input when done.")
            .build();
        let source = widgets::dropdown(&["Analog input", "Pink noise", "White noise", "Sine"]);
        let freq_names: Vec<String> = GEQ_BANDS_HZ.iter().map(|f| format::hz(*f)).collect();
        let freq_refs: Vec<&str> = freq_names.iter().map(String::as_str).collect();
        let freq = widgets::dropdown(&freq_refs);
        freq.set_selected(17);
        freq.set_sensitive(false);
        let send_tone = {
            let (source, freq) = (source.clone(), freq.clone());
            move |app: &App| {
                let s = source.selected() as u8;
                freq.set_sensitive(s == ToneSource::Sine as u8);
                if app.is_connected() {
                    app.send(Request::TestTone {
                        source: s,
                        freq: freq.selected() as u8,
                    });
                } else {
                    app.toast("Not connected");
                }
            }
        };
        let send_tone = Rc::new(send_tone);
        {
            let (app, f) = (Rc::clone(app), Rc::clone(&send_tone));
            source.connect_selected_notify(move |_| f(&app));
        }
        {
            let (app, f) = (Rc::clone(app), Rc::clone(&send_tone));
            freq.connect_selected_notify(move |_| f(&app));
        }
        tone.add(&row("Source", None, &source));
        tone.add(&row("Sine frequency", None, &freq));
        root.add(&tone);

        let display = adw::PreferencesGroup::builder().title("Display").build();
        let delay_unit = widgets::dropdown(&["Milliseconds", "Metres", "Feet"]);
        {
            let app = Rc::clone(app);
            delay_unit.connect_selected_notify(move |d| {
                app.edit(Request::SetDelayUnit(d.selected() as u8))
            });
        }
        display.add(&row("Delay unit", Some("stored on the unit"), &delay_unit));
        root.add(&display);

        DevicePage {
            root,
            address,
            device_id,
            usb,
            connect_usb,
            connect,
            demo,
            disconnect,
            info,
            presets,
            delay_unit,
        }
    }

    pub fn refresh(&self, app: &App) {
        let m = app.model.borrow();
        let kind = app.kind.get();
        let online = kind.is_online();
        let usb = app.usb_devices.borrow();
        self.usb.set_subtitle(&format::usb_devices(&usb));
        self.connect_usb
            .set_sensitive(kind == LinkKind::Offline && !usb.is_empty());
        self.connect.set_sensitive(kind == LinkKind::Offline);
        self.demo.set_sensitive(kind == LinkKind::Offline);
        self.disconnect.set_sensitive(online);
        self.address.set_sensitive(kind == LinkKind::Offline);
        self.device_id.set_sensitive(kind == LinkKind::Offline);
        self.info[0].set_subtitle(m.identity.as_deref().unwrap_or("unknown"));
        self.info[1].set_subtitle(match kind {
            LinkKind::Offline => "offline",
            LinkKind::Connecting => "connecting",
            LinkKind::Device => "network",
            LinkKind::Usb => "USB",
            LinkKind::Demo => "simulated device",
        });
        self.info[2].set_subtitle(
            &m.current_preset
                .map(Model::preset_label)
                .unwrap_or_else(|| "unknown".into()),
        );
        self.info[3].set_subtitle(&m.program.name_text());
        for (n, r) in self.presets.iter().enumerate() {
            let name = &m.preset_names[n];
            r.row.set_subtitle(if name.is_empty() { "—" } else { name });
            r.store.set_sensitive(online);
        }
        widgets::set_drop(
            &self.delay_unit,
            u32::from(m.program.globals.delay_unit.min(2)),
        );
    }
}

fn store_dialog(app: &Rc<App>, n: u8) {
    let Some(ui) = app.ui() else { return };
    let entry = gtk::Entry::builder()
        .max_length(14)
        .text(app.model.borrow().program.name_text())
        .activates_default(true)
        .build();
    let dialog = adw::AlertDialog::new(
        Some(&format!("Store preset {}", Model::preset_label(n))),
        Some("The current program will overwrite this preset on the unit. Name (14 characters):"),
    );
    dialog.set_extra_child(Some(&entry));
    dialog.add_responses(&[("cancel", "Cancel"), ("store", "Store")]);
    dialog.set_response_appearance("store", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("store"));
    dialog.set_close_response("cancel");
    let app = Rc::clone(app);
    dialog.connect_response(None, move |_, resp| {
        if resp == "store" {
            app.store_preset(n, &entry.text());
        }
    });
    dialog.present(Some(&ui.window));
}

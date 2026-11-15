//! Mixer page: one strip per channel with meter, fader, mute, polarity and
//! (outputs) routing.

use std::rc::Rc;

use adw::prelude::*;
use dsp206_proto::units::GAIN_MAX;
use dsp206_proto::{Channel, Request};

use super::{draw, widgets};
use crate::app::App;
use crate::format;

struct Strip {
    ch: Channel,
    frame: gtk::Box,
    name: gtk::Label,
    meter: gtk::DrawingArea,
    fader: gtk::Scale,
    value: gtk::Label,
    mute: gtk::ToggleButton,
    polarity: gtk::ToggleButton,
    route: Option<(gtk::ToggleButton, gtk::ToggleButton)>,
    info: gtk::Label,
}

pub struct Mixer {
    pub root: gtk::ScrolledWindow,
    strips: Vec<Strip>,
}

fn meter_area(app: &Rc<App>, ch: Channel) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(14)
        .vexpand(true)
        .build();
    let app = Rc::clone(app);
    area.set_draw_func(move |a, cr, w, h| {
        let m = app.meters.borrow();
        let v = m.as_ref().map(|m| {
            if ch.is_input() {
                m.inputs[ch.index() as usize]
            } else {
                m.outputs[ch.index() as usize - 2]
            }
        });
        draw::meter(cr, f64::from(w), f64::from(h), v, &a.color());
    });
    area
}

impl Mixer {
    pub fn build(app: &Rc<App>) -> Mixer {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_margin_top(14);
        row.set_margin_bottom(14);
        row.set_margin_start(14);
        row.set_margin_end(14);
        let mut strips = Vec::new();
        for ch in Channel::all() {
            if ch.index() == 2 {
                row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
            }
            let s = Self::strip(app, ch);
            row.append(&s.frame);
            strips.push(s);
        }
        let root = gtk::ScrolledWindow::builder()
            .child(&row)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
        Mixer { root, strips }
    }

    fn strip(app: &Rc<App>, ch: Channel) -> Strip {
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 8);
        frame.add_css_class("strip");
        frame.set_size_request(132, -1);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let name = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        name.add_css_class("strip-name");
        let tag = gtk::Label::new(Some(&ch.label()));
        tag.add_css_class("dim-label");
        tag.add_css_class("caption");
        head.append(&name);
        head.append(&tag);
        frame.append(&head);

        let info = gtk::Label::builder().xalign(0.0).wrap(true).build();
        info.add_css_class("caption");
        info.add_css_class("dim-label");
        frame.append(&info);

        let route = if ch.is_input() {
            None
        } else {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let a_btn = gtk::ToggleButton::with_label("A");
            let b_btn = gtk::ToggleButton::with_label("B");
            a_btn.set_tooltip_text(Some("Route input A to this output"));
            b_btn.set_tooltip_text(Some("Route input B to this output"));
            for btn in [&a_btn, &b_btn] {
                let app = Rc::clone(app);
                let (a2, b2) = (a_btn.clone(), b_btn.clone());
                btn.connect_toggled(move |_| {
                    let mask = u8::from(a2.is_active()) | (u8::from(b2.is_active()) << 1);
                    app.edit(Request::SetRouting { ch, mask });
                });
                b.append(btn);
            }
            frame.append(&b);
            Some((a_btn, b_btn))
        };

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        body.set_vexpand(true);
        body.set_halign(gtk::Align::Center);
        let meter = meter_area(app, ch);
        let fader =
            gtk::Scale::with_range(gtk::Orientation::Vertical, 0.0, f64::from(GAIN_MAX), 1.0);
        fader.set_inverted(true);
        fader.set_vexpand(true);
        fader.set_size_request(-1, 280);
        fader.set_draw_value(false);
        fader.set_increments(1.0, 10.0);
        for (raw, label) in [
            (400.0, "+12"),
            (280.0, "0"),
            (180.0, "-10"),
            (80.0, "-20"),
            (40.0, "-40"),
            (0.0, "-60"),
        ] {
            fader.add_mark(raw, gtk::PositionType::Left, Some(label));
        }
        {
            let app = Rc::clone(app);
            fader.connect_value_changed(move |s| {
                app.edit(Request::SetGain {
                    ch,
                    raw: s.value().round() as u16,
                })
            });
        }
        body.append(&meter);
        body.append(&fader);
        frame.append(&body);

        let value = gtk::Label::new(None);
        value.add_css_class("title-4");
        value.add_css_class("value");
        frame.append(&value);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        buttons.set_halign(gtk::Align::Center);
        let mute = gtk::ToggleButton::with_label("M");
        mute.set_tooltip_text(Some("Mute"));
        let polarity = gtk::ToggleButton::with_label("Ø");
        polarity.set_tooltip_text(Some("Invert polarity"));
        let edit = gtk::Button::from_icon_name("document-edit-symbolic");
        edit.set_tooltip_text(Some("Open in the channel editor"));
        {
            let app = Rc::clone(app);
            mute.connect_toggled(move |t| {
                app.edit(Request::SetMute {
                    ch,
                    on: t.is_active(),
                })
            });
        }
        {
            let app = Rc::clone(app);
            polarity.connect_toggled(move |t| {
                app.edit(Request::SetPolarity {
                    ch,
                    inverted: t.is_active(),
                })
            });
        }
        {
            let app = Rc::clone(app);
            edit.connect_clicked(move |_| {
                app.select_channel(ch);
                if let Some(ui) = app.ui() {
                    ui.stack.set_visible_child_name("channel");
                }
            });
        }
        buttons.append(&mute);
        buttons.append(&polarity);
        buttons.append(&edit);
        frame.append(&buttons);

        Strip {
            ch,
            frame,
            name,
            meter,
            fader,
            value,
            mute,
            polarity,
            route,
            info,
        }
    }

    pub fn refresh(&self, app: &App) {
        let m = app.model.borrow();
        let p = &m.program;
        for s in &self.strips {
            let st = p.strip(s.ch);
            s.name.set_text(&p.channel_name(s.ch));
            widgets::set_scale(&s.fader, st.gain);
            s.value.set_text(&format::gain(st.gain));
            let muted = p.is_muted(s.ch);
            widgets::set_toggle(&s.mute, muted);
            widgets::set_class(&s.mute, "mute-on", muted);
            widgets::set_toggle(&s.polarity, st.polarity != 0);
            widgets::set_class(&s.polarity, "toggle-on", st.polarity != 0);
            widgets::set_class(&s.frame, "selected", app.selected.get() == s.ch);
            let mut info = vec![
                format!("HP {}", format::freq_index(st.high_pass.freq)),
                format!("LP {}", format::freq_index(st.low_pass.freq)),
                format!("Delay {}", format::delay(st.delay, p.globals.delay_unit)),
            ];
            let linked: Vec<String> = p
                .linked_channels(s.ch)
                .iter()
                .filter(|c| **c != s.ch)
                .map(|c| c.label())
                .collect();
            if !linked.is_empty() {
                info.push(format!("Linked: {}", linked.join(", ")));
            }
            if p.peq_bypassed(s.ch) {
                info.push("PEQ bypassed".into());
            }
            s.info.set_text(&info.join("\n"));
            if let Some((a, b)) = &s.route {
                let r = p.outputs[s.ch.index() as usize - 2].routing;
                widgets::set_toggle(a, r & 1 != 0);
                widgets::set_toggle(b, r & 2 != 0);
                widgets::set_class(a, "toggle-on", r & 1 != 0);
                widgets::set_class(b, "toggle-on", r & 2 != 0);
            }
        }
    }

    pub fn redraw_meters(&self) {
        for s in &self.strips {
            s.meter.queue_draw();
        }
    }
}

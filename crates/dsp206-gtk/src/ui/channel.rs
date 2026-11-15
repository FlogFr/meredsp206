//! Channel editor: EQ graph, PEQ table, GEQ, filters, dynamics, routing.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use dsp206_proto::edit::domain_bit;
use dsp206_proto::program::{name_field, CutFilter, PeqBand, GEQ_BANDS};
use dsp206_proto::units::{self, PeqType};
use dsp206_proto::{Channel, Request};
use gtk::glib;

use super::draw::{self, Graph};
use super::widgets::{self, raw_spin, row};
use crate::app::App;
use crate::format;

struct BandRow {
    select: gtk::ToggleButton,
    kind: gtk::DropDown,
    freq: gtk::SpinButton,
    gain: gtk::SpinButton,
    q: gtk::SpinButton,
    bypass: gtk::CheckButton,
}

pub struct ChannelPage {
    pub root: gtk::Box,
    list: gtk::ListBox,
    list_labels: Vec<gtk::Label>,
    title: gtk::Label,
    graph: gtk::DrawingArea,
    meter: gtk::DrawingArea,
    peq_bypass: gtk::ToggleButton,
    tabs: gtk::Stack,
    bands: Vec<BandRow>,
    geq_page: gtk::Box,
    geq: Vec<(gtk::Scale, gtk::Label)>,
    geq_bypass: gtk::ToggleButton,
    name: gtk::Entry,
    gain: gtk::SpinButton,
    mute: gtk::Switch,
    polarity: gtk::Switch,
    delay: gtk::SpinButton,
    link_buttons: Vec<gtk::ToggleButton>,
    copy_buttons: Vec<gtk::Button>,
    hp_freq: gtk::SpinButton,
    hp_slope: gtk::DropDown,
    lp_freq: gtk::SpinButton,
    lp_slope: gtk::DropDown,
    routing: adw::PreferencesGroup,
    route: [(gtk::Switch, gtk::SpinButton); 2],
    gate: adw::PreferencesGroup,
    gate_w: [gtk::SpinButton; 4],
    comp: adw::PreferencesGroup,
    comp_ratio: gtk::DropDown,
    comp_w: [gtk::SpinButton; 4],
    limiter: adw::PreferencesGroup,
    lim_w: [gtk::SpinButton; 4],
}

fn parse_freq(t: &str) -> Option<u16> {
    format::parse_number(t).map(units::freq_index)
}
fn parse_gain(t: &str) -> Option<u16> {
    format::parse_number(t).map(units::gain_raw)
}
fn parse_eq_gain(t: &str) -> Option<u16> {
    format::parse_number(t).map(units::eq_gain_raw)
}
fn parse_ms(t: &str) -> Option<u16> {
    format::parse_number(t).map(|v| (v.round().max(1.0) - 1.0).min(f64::from(u16::MAX)) as u16)
}
fn freq_spin() -> gtk::SpinButton {
    raw_spin(0, units::FREQ_INDEX_MAX, 10, format::freq_index, parse_freq)
}
fn time_spin(min: u16, max: u16) -> gtk::SpinButton {
    raw_spin(min, max, 10, |r| format::ms(units::time_ms(r)), parse_ms)
}
fn threshold_spin(max: u16) -> gtk::SpinButton {
    raw_spin(0, max, 10, format::threshold, move |t| {
        format::parse_number(t).map(|db| units::threshold_raw(db, max))
    })
}

impl ChannelPage {
    pub fn build(app: &Rc<App>) -> ChannelPage {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        // ---- channel list ----
        let list = gtk::ListBox::new();
        list.add_css_class("navigation-sidebar");
        let mut list_labels = Vec::new();
        for _ in Channel::all() {
            let l = gtk::Label::builder()
                .xalign(0.0)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .build();
            list.append(&l);
            list_labels.push(l);
        }
        {
            let app = Rc::clone(app);
            list.connect_row_selected(move |_, r| {
                if let Some(ch) = r.and_then(|r| Channel::new(r.index() as u8)) {
                    if !app.updating.get() {
                        app.select_channel(ch);
                    }
                }
            });
        }
        let side = gtk::ScrolledWindow::builder()
            .child(&list)
            .width_request(190)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        root.append(&side);
        root.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        // ---- centre column ----
        let centre = gtk::Box::new(gtk::Orientation::Vertical, 10);
        centre.set_hexpand(true);
        centre.set_margin_top(12);
        centre.set_margin_bottom(12);
        centre.set_margin_start(12);
        centre.set_margin_end(12);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::builder().xalign(0.0).hexpand(true).build();
        title.add_css_class("title-2");
        let peq_bypass = gtk::ToggleButton::with_label("PEQ bypass");
        let reset = gtk::Button::with_label("Flat PEQ");
        reset.set_tooltip_text(Some("Set every PEQ band gain to 0 dB"));
        head.append(&title);
        head.append(&reset);
        head.append(&peq_bypass);
        centre.append(&head);
        {
            let app = Rc::clone(app);
            peq_bypass.connect_toggled(move |t| {
                app.edit(Request::SetPeqBypass {
                    ch: app.selected.get(),
                    on: t.is_active(),
                })
            });
        }
        {
            let app = Rc::clone(app);
            reset.connect_clicked(move |_| {
                let ch = app.selected.get();
                let bands: Vec<(u8, PeqBand, bool)> = {
                    let m = app.model.borrow();
                    let p = &m.program;
                    p.strip(ch)
                        .peq
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (i as u8, *b, p.peq_band_bypassed(ch, i as u8)))
                        .collect()
                };
                for (band, mut value, bypass) in bands {
                    value.gain = units::EQ_GAIN_FLAT;
                    app.edit(Request::SetPeqBand {
                        ch,
                        band,
                        value,
                        bypass,
                    });
                }
            });
        }

        let graph_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let graph = gtk::DrawingArea::builder()
            .content_height(300)
            .hexpand(true)
            .build();
        {
            let app = Rc::clone(app);
            graph.set_draw_func(move |a, cr, w, h| {
                let m = app.model.borrow();
                let g = Graph {
                    w: f64::from(w),
                    h: f64::from(h),
                };
                let accent = adw::StyleManager::default().accent_color_rgba();
                draw::eq_graph(
                    cr,
                    g,
                    &m.program,
                    app.selected.get(),
                    app.selected_band.get(),
                    &a.color(),
                    &accent,
                );
            });
        }
        Self::graph_input(app, &graph);
        let meter = gtk::DrawingArea::builder().content_width(14).build();
        {
            let app = Rc::clone(app);
            meter.set_draw_func(move |a, cr, w, h| {
                let ch = app.selected.get();
                let v = app.meters.borrow().as_ref().map(|m| {
                    if ch.is_input() {
                        m.inputs[ch.index() as usize]
                    } else {
                        m.outputs[ch.index() as usize - 2]
                    }
                });
                draw::meter(cr, f64::from(w), f64::from(h), v, &a.color());
            });
        }
        graph_row.append(&graph);
        graph_row.append(&meter);
        centre.append(&graph_row);

        let tabs = gtk::Stack::new();
        let switcher = gtk::StackSwitcher::builder()
            .stack(&tabs)
            .halign(gtk::Align::Start)
            .build();
        centre.append(&switcher);

        // PEQ table
        let grid = gtk::Grid::builder()
            .row_spacing(4)
            .column_spacing(10)
            .build();
        for (c, h) in ["Band", "Type", "Frequency", "Gain", "Q", "Bypass"]
            .iter()
            .enumerate()
        {
            let l = gtk::Label::builder().label(*h).xalign(0.0).build();
            l.add_css_class("dim-label");
            l.add_css_class("caption-heading");
            grid.attach(&l, c as i32, 0, 1, 1);
        }
        let type_names: Vec<&str> = PeqType::ALL.iter().map(|t| t.label()).collect();
        let mut bands = Vec::new();
        for i in 0..9usize {
            let select = gtk::ToggleButton::with_label(&format!("{}", i + 1));
            let kind = widgets::dropdown(&type_names);
            let freq = freq_spin();
            let gain = raw_spin(0, units::EQ_GAIN_MAX, 10, format::eq_gain, parse_eq_gain);
            let q = raw_spin(
                0,
                100,
                5,
                |r| format::q(r as u8),
                |t| format::parse_number(t).map(|v| u16::from(units::q_index(v))),
            );
            let bypass = gtk::CheckButton::new();
            bypass.set_halign(gtk::Align::Center);
            let r = i as i32 + 1;
            grid.attach(&select, 0, r, 1, 1);
            grid.attach(&kind, 1, r, 1, 1);
            grid.attach(&freq, 2, r, 1, 1);
            grid.attach(&gain, 3, r, 1, 1);
            grid.attach(&q, 4, r, 1, 1);
            grid.attach(&bypass, 5, r, 1, 1);
            Self::band_handlers(app, i, &select, &kind, &freq, &gain, &q, &bypass);
            bands.push(BandRow {
                select,
                kind,
                freq,
                gain,
                q,
                bypass,
            });
        }
        let peq_page = gtk::ScrolledWindow::builder()
            .child(&grid)
            .vexpand(true)
            .build();
        tabs.add_titled(&peq_page, Some("peq"), "Parametric EQ");

        // GEQ
        let geq_page = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let geq_head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let flat = gtk::Button::with_label("Flatten");
        let geq_bypass = gtk::ToggleButton::with_label("GEQ bypass");
        geq_head.append(&flat);
        geq_head.append(&geq_bypass);
        geq_page.append(&geq_head);
        let sliders = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        sliders.set_vexpand(true);
        let mut geq = Vec::new();
        for (i, f) in units::GEQ_BANDS_HZ.iter().enumerate() {
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let value = gtk::Label::new(None);
            value.add_css_class("caption");
            value.add_css_class("value");
            let s = gtk::Scale::with_range(
                gtk::Orientation::Vertical,
                0.0,
                f64::from(units::EQ_GAIN_MAX),
                1.0,
            );
            s.set_inverted(true);
            s.set_vexpand(true);
            s.set_draw_value(false);
            s.add_mark(
                f64::from(units::EQ_GAIN_FLAT),
                gtk::PositionType::Left,
                None,
            );
            s.set_tooltip_text(Some(&format::hz(*f)));
            let label = gtk::Label::new(Some(&if *f >= 1000.0 {
                format!("{}k", f / 1000.0)
            } else {
                format!("{f}")
            }));
            label.add_css_class("caption");
            col.append(&value);
            col.append(&s);
            col.append(&label);
            sliders.append(&col);
            let app = Rc::clone(app);
            s.connect_value_changed(move |s| {
                let ch = app.selected.get();
                if ch.is_input() {
                    app.edit(Request::SetGeqBand {
                        ch,
                        band: i as u8,
                        gain: s.value().round() as u16,
                    });
                }
            });
            geq.push((s, value));
        }
        geq_page.append(&sliders);
        {
            let app = Rc::clone(app);
            geq_bypass.connect_toggled(move |t| {
                app.edit(Request::SetGeqBypass {
                    ch: app.selected.get(),
                    on: t.is_active(),
                })
            });
        }
        {
            let app = Rc::clone(app);
            flat.connect_clicked(move |_| {
                let ch = app.selected.get();
                if ch.is_input() {
                    for band in 0..GEQ_BANDS as u8 {
                        app.edit(Request::SetGeqBand {
                            ch,
                            band,
                            gain: units::EQ_GAIN_FLAT,
                        });
                    }
                }
            });
        }
        tabs.add_titled(&geq_page, Some("geq"), "Graphic EQ");
        centre.append(&tabs);
        root.append(&centre);
        root.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        // ---- right column ----
        let right = gtk::Box::new(gtk::Orientation::Vertical, 18);
        right.set_margin_top(12);
        right.set_margin_bottom(12);
        right.set_margin_start(12);
        right.set_margin_end(12);

        let chan = adw::PreferencesGroup::builder().title("Channel").build();
        let name = gtk::Entry::builder()
            .max_length(8)
            .width_chars(10)
            .valign(gtk::Align::Center)
            .build();
        {
            let app2 = Rc::clone(app);
            name.connect_activate(move |e| {
                app2.edit(Request::SetChannelName {
                    ch: app2.selected.get(),
                    name: name_field(&e.text()),
                });
            });
            let focus = gtk::EventControllerFocus::new();
            let app3 = Rc::clone(app);
            let e = name.clone();
            focus.connect_leave(move |_| {
                app3.edit(Request::SetChannelName {
                    ch: app3.selected.get(),
                    name: name_field(&e.text()),
                });
            });
            name.add_controller(focus);
        }
        chan.add(&row("Name", Some("8 characters"), &name));
        let gain = raw_spin(0, units::GAIN_MAX, 10, format::gain, parse_gain);
        {
            let app = Rc::clone(app);
            gain.connect_value_changed(move |s| {
                app.edit(Request::SetGain {
                    ch: app.selected.get(),
                    raw: s.value().round() as u16,
                })
            });
        }
        chan.add(&row("Gain", None, &gain));
        let mute = widgets::switch();
        {
            let app = Rc::clone(app);
            mute.connect_active_notify(move |s| {
                app.edit(Request::SetMute {
                    ch: app.selected.get(),
                    on: s.is_active(),
                })
            });
        }
        chan.add(&row("Mute", None, &mute));
        let polarity = widgets::switch();
        {
            let app = Rc::clone(app);
            polarity.connect_active_notify(move |s| {
                app.edit(Request::SetPolarity {
                    ch: app.selected.get(),
                    inverted: s.is_active(),
                })
            });
        }
        chan.add(&row("Invert polarity", None, &polarity));
        let delay = {
            let (a1, a2) = (Rc::clone(app), Rc::clone(app));
            raw_spin(
                0,
                units::DELAY_MAX,
                96,
                move |r| format::delay(r, a1.model.borrow().program.globals.delay_unit),
                move |t| {
                    let v = format::parse_number(t)?;
                    let ms = match a2.model.borrow().program.globals.delay_unit {
                        1 => v / units::SPEED_OF_SOUND_M_PER_MS,
                        2 => v / units::FEET_PER_METER / units::SPEED_OF_SOUND_M_PER_MS,
                        _ => v,
                    };
                    Some(units::delay_raw(ms))
                },
            )
        };
        {
            let app = Rc::clone(app);
            delay.connect_value_changed(move |s| {
                app.edit(Request::SetDelay {
                    ch: app.selected.get(),
                    raw: s.value().round() as u16,
                })
            });
        }
        chan.add(&row("Delay", Some("up to 680 ms"), &delay));

        let link_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        link_box.set_valign(gtk::Align::Center);
        let mut link_buttons = Vec::new();
        for other in Channel::all() {
            let b = gtk::ToggleButton::with_label(&other.label());
            {
                let app = Rc::clone(app);
                b.connect_toggled(move |t| {
                    if !app.updating.get() {
                        set_link(&app, app.selected.get(), other, t.is_active());
                    }
                });
            }
            link_box.append(&b);
            link_buttons.push(b);
        }
        chan.add(&row(
            "Linked with",
            Some("edits apply to the whole group"),
            &link_box,
        ));

        let copy_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut copy_buttons = Vec::new();
        for dst in Channel::all() {
            let b = gtk::Button::with_label(&dst.label());
            b.add_css_class("flat");
            {
                let app = Rc::clone(app);
                b.connect_clicked(move |_| {
                    let src = app.selected.get();
                    app.edit(Request::CopyChannel { src, dst });
                    app.toast(&format!("Copied {} to {}", src.label(), dst.label()));
                });
            }
            copy_list.append(&b);
            copy_buttons.push(b);
        }
        let copy_pop = gtk::Popover::builder().child(&copy_list).build();
        let copy = gtk::MenuButton::builder()
            .label("Copy to…")
            .popover(&copy_pop)
            .valign(gtk::Align::Center)
            .build();
        chan.add(&row(
            "Copy settings",
            Some("keeps the destination name and routing"),
            &copy,
        ));
        right.append(&chan);

        let slope_names: Vec<String> = (0..20).filter_map(units::slope_label).collect();
        let slope_refs: Vec<&str> = slope_names.iter().map(String::as_str).collect();
        let filters = adw::PreferencesGroup::builder().title("Crossover").build();
        let hp_freq = freq_spin();
        let hp_slope = widgets::dropdown(&slope_refs);
        let lp_freq = freq_spin();
        let lp_slope = widgets::dropdown(&slope_refs);
        for (high, f, s) in [(true, &hp_freq, &hp_slope), (false, &lp_freq, &lp_slope)] {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            b.append(f);
            b.append(s);
            filters.add(&row(if high { "High-pass" } else { "Low-pass" }, None, &b));
            let (f2, s2) = (f.clone(), s.clone());
            let make = move |app: &App| {
                let filter = CutFilter {
                    freq: f2.value().round() as u16,
                    slope: s2.selected() as u8,
                };
                let ch = app.selected.get();
                app.edit(if high {
                    Request::SetHighPass { ch, filter }
                } else {
                    Request::SetLowPass { ch, filter }
                });
            };
            let make = Rc::new(make);
            {
                let (app, make) = (Rc::clone(app), Rc::clone(&make));
                f.connect_value_changed(move |_| make(&app));
            }
            {
                let (app, make) = (Rc::clone(app), Rc::clone(&make));
                s.connect_selected_notify(move |_| make(&app));
            }
        }
        right.append(&filters);

        // Routing (outputs)
        let routing = adw::PreferencesGroup::builder()
            .title("Inputs to this output")
            .build();
        let route = [0u8, 1].map(|input| {
            let sw = widgets::switch();
            let lvl = raw_spin(0, units::MIX_GAIN_MAX, 10, format::gain, parse_gain);
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            b.append(&lvl);
            b.append(&sw);
            routing.add(&row(
                if input == 0 { "Input A" } else { "Input B" },
                Some("level and on/off"),
                &b,
            ));
            {
                let app = Rc::clone(app);
                sw.connect_active_notify(move |s| {
                    let ch = app.selected.get();
                    if ch.is_input() {
                        return;
                    }
                    let cur =
                        app.model.borrow().program.outputs[ch.index() as usize - 2].routing as u8;
                    let mask = if s.is_active() {
                        cur | (1 << input)
                    } else {
                        cur & !(1 << input)
                    };
                    app.edit(Request::SetRouting { ch, mask });
                });
            }
            {
                let app = Rc::clone(app);
                lvl.connect_value_changed(move |s| {
                    let ch = app.selected.get();
                    if !ch.is_input() {
                        app.edit(Request::SetMixGain {
                            ch,
                            input,
                            raw: s.value().round() as u16,
                        });
                    }
                });
            }
            (sw, lvl)
        });
        right.append(&routing);

        // Gate (inputs)
        let gate = adw::PreferencesGroup::builder().title("Noise gate").build();
        let gate_w = [
            threshold_spin(units::GATE_THRESHOLD_MAX),
            time_spin(0, units::ATTACK_MAX),
            time_spin(units::HOLD_MIN, units::HOLD_MAX),
            time_spin(units::RELEASE_MIN, units::RELEASE_MAX),
        ];
        for (title, w) in ["Threshold", "Attack", "Hold", "Release"]
            .iter()
            .zip(&gate_w)
        {
            gate.add(&row(title, None, w));
        }
        {
            let ws = gate_w.clone();
            for w in &gate_w {
                let (app, ws) = (Rc::clone(app), ws.clone());
                w.connect_value_changed(move |_| {
                    let v = |i: usize| ws[i].value().round() as u16;
                    let ch = app.selected.get();
                    if ch.is_input() {
                        let g = dsp206_proto::program::Gate {
                            threshold: v(0),
                            attack: v(1),
                            hold: v(2),
                            release: v(3),
                        };
                        app.edit(Request::SetGate { ch, gate: g });
                    }
                });
            }
        }
        right.append(&gate);

        // Compressor (outputs)
        let comp = adw::PreferencesGroup::builder()
            .title("RMS compressor")
            .build();
        let ratio_names: Vec<String> = (0..16).map(format::ratio).collect();
        let ratio_refs: Vec<&str> = ratio_names.iter().map(String::as_str).collect();
        let comp_ratio = widgets::dropdown(&ratio_refs);
        let comp_w = [
            threshold_spin(units::DYN_THRESHOLD_MAX),
            time_spin(0, units::ATTACK_MAX),
            time_spin(units::RELEASE_MIN, units::RELEASE_MAX),
            raw_spin(
                0,
                units::KNEE_MAX,
                1,
                |r| format!("{r} dB"),
                |t| format::parse_number(t).map(|v| v.round().clamp(0.0, 12.0) as u16),
            ),
        ];
        comp.add(&row("Threshold", None, &comp_w[0]));
        comp.add(&row("Ratio", None, &comp_ratio));
        comp.add(&row("Attack", None, &comp_w[1]));
        comp.add(&row("Release", None, &comp_w[2]));
        comp.add(&row("Knee", None, &comp_w[3]));
        {
            let ws = comp_w.clone();
            let send = {
                let ratio = comp_ratio.clone();
                move |app: &App| {
                    let v = |i: usize| ws[i].value().round() as u16;
                    let ch = app.selected.get();
                    if !ch.is_input() {
                        let c = dsp206_proto::program::Compressor {
                            threshold: v(0),
                            attack: v(1),
                            release: v(2),
                            knee: v(3),
                            ratio: ratio.selected() as u16,
                        };
                        app.edit(Request::SetCompressor { ch, comp: c });
                    }
                }
            };
            let send = Rc::new(send);
            for w in &comp_w {
                let (app, send) = (Rc::clone(app), Rc::clone(&send));
                w.connect_value_changed(move |_| send(&app));
            }
            let (app, send) = (Rc::clone(app), Rc::clone(&send));
            comp_ratio.connect_selected_notify(move |_| send(&app));
        }
        right.append(&comp);

        // Limiter (outputs)
        let limiter = adw::PreferencesGroup::builder()
            .title("Peak limiter")
            .build();
        let lim_w = [
            threshold_spin(units::DYN_THRESHOLD_MAX),
            time_spin(0, units::ATTACK_MAX),
            time_spin(units::RELEASE_MIN, units::RELEASE_MAX),
            raw_spin(
                0,
                units::KNEE_MAX,
                1,
                |r| format!("{r} dB"),
                |t| format::parse_number(t).map(|v| v.round().clamp(0.0, 12.0) as u16),
            ),
        ];
        for (title, w) in ["Threshold", "Attack", "Release", "Knee"]
            .iter()
            .zip(&lim_w)
        {
            limiter.add(&row(title, None, w));
        }
        {
            let ws = lim_w.clone();
            for w in &lim_w {
                let (app, ws) = (Rc::clone(app), ws.clone());
                w.connect_value_changed(move |_| {
                    let v = |i: usize| ws[i].value().round() as u16;
                    let ch = app.selected.get();
                    if !ch.is_input() {
                        let l = dsp206_proto::program::Limiter {
                            threshold: v(0),
                            attack: v(1),
                            release: v(2),
                            knee: v(3),
                        };
                        app.edit(Request::SetLimiter { ch, limiter: l });
                    }
                });
            }
        }
        right.append(&limiter);

        let right_scroll = gtk::ScrolledWindow::builder()
            .child(&right)
            .width_request(430)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        root.append(&right_scroll);

        ChannelPage {
            root,
            list,
            list_labels,
            title,
            graph,
            meter,
            peq_bypass,
            tabs,
            bands,
            geq_page,
            geq,
            geq_bypass,
            name,
            gain,
            mute,
            polarity,
            delay,
            link_buttons,
            copy_buttons,
            hp_freq,
            hp_slope,
            lp_freq,
            lp_slope,
            routing,
            route,
            gate,
            gate_w,
            comp,
            comp_ratio,
            comp_w,
            limiter,
            lim_w,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn band_handlers(
        app: &Rc<App>,
        i: usize,
        select: &gtk::ToggleButton,
        kind: &gtk::DropDown,
        freq: &gtk::SpinButton,
        gain: &gtk::SpinButton,
        q: &gtk::SpinButton,
        bypass: &gtk::CheckButton,
    ) {
        {
            let app = Rc::clone(app);
            select.connect_clicked(move |_| {
                if !app.updating.get() {
                    app.selected_band.set(i);
                    app.refresh();
                }
            });
        }
        let (k, f, g, qq, b) = (
            kind.clone(),
            freq.clone(),
            gain.clone(),
            q.clone(),
            bypass.clone(),
        );
        let send = Rc::new(move |app: &App| {
            let ch = app.selected.get();
            let value = PeqBand {
                kind: k.selected() as u8,
                freq: f.value().round() as u16,
                gain: g.value().round() as u16,
                q: qq.value().round() as u8,
            };
            if !app.updating.get() {
                app.selected_band.set(i);
            }
            app.edit(Request::SetPeqBand {
                ch,
                band: i as u8,
                value,
                bypass: b.is_active(),
            });
        });
        for spin in [freq, gain, q] {
            let (app, send) = (Rc::clone(app), Rc::clone(&send));
            spin.connect_value_changed(move |_| send(&app));
        }
        {
            let (app, send) = (Rc::clone(app), Rc::clone(&send));
            kind.connect_selected_notify(move |_| send(&app));
        }
        let (app, send) = (Rc::clone(app), Rc::clone(&send));
        bypass.connect_toggled(move |_| send(&app));
    }

    /// Drag band handles on the graph; scroll changes Q.
    fn graph_input(app: &Rc<App>, graph: &gtk::DrawingArea) {
        let drag = gtk::GestureDrag::new();
        let grabbed: Rc<Cell<Option<(usize, f64, f64)>>> = Rc::new(Cell::new(None));
        {
            let (app, grabbed, area) = (Rc::clone(app), Rc::clone(&grabbed), graph.clone());
            drag.connect_drag_begin(move |_, x, y| {
                let g = Graph {
                    w: f64::from(area.width()),
                    h: f64::from(area.height()),
                };
                let ch = app.selected.get();
                let best = {
                    let m = app.model.borrow();
                    (0..m.program.strip(ch).peq.len())
                        .filter_map(|i| {
                            g.handle(&m.program, ch, i)
                                .map(|(hx, hy)| (i, (hx - x).hypot(hy - y), hx, hy))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                };
                match best {
                    Some((i, d, hx, hy)) if d < 18.0 => {
                        grabbed.set(Some((i, hx, hy)));
                        app.selected_band.set(i);
                        app.refresh();
                    }
                    _ => grabbed.set(None),
                }
            });
        }
        {
            let (app, grabbed, area) = (Rc::clone(app), Rc::clone(&grabbed), graph.clone());
            drag.connect_drag_update(move |_, dx, dy| {
                let Some((i, hx, hy)) = grabbed.get() else {
                    return;
                };
                let g = Graph {
                    w: f64::from(area.width()),
                    h: f64::from(area.height()),
                };
                let ch = app.selected.get();
                let (mut value, bypass) = {
                    let m = app.model.borrow();
                    let Some(b) = m.program.strip(ch).peq.get(i).copied() else {
                        return;
                    };
                    (b, m.program.peq_band_bypassed(ch, i as u8))
                };
                value.freq = units::freq_index(g.freq_at((hx + dx).clamp(0.0, g.w)));
                if matches!(
                    PeqType::from_raw(value.kind),
                    Some(PeqType::Peak | PeqType::LowShelf | PeqType::HighShelf)
                ) {
                    value.gain = units::eq_gain_raw(g.db_at(hy + dy));
                }
                app.edit(Request::SetPeqBand {
                    ch,
                    band: i as u8,
                    value,
                    bypass,
                });
            });
        }
        graph.add_controller(drag);
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        {
            let app = Rc::clone(app);
            scroll.connect_scroll(move |_, _, dy| {
                let ch = app.selected.get();
                let i = app.selected_band.get();
                let (mut value, bypass) = {
                    let m = app.model.borrow();
                    let Some(b) = m.program.strip(ch).peq.get(i).copied() else {
                        return glib::Propagation::Proceed;
                    };
                    (b, m.program.peq_band_bypassed(ch, i as u8))
                };
                let step: i16 = if dy < 0.0 { 1 } else { -1 };
                value.q = (i16::from(value.q) + step).clamp(0, 100) as u8;
                app.edit(Request::SetPeqBand {
                    ch,
                    band: i as u8,
                    value,
                    bypass,
                });
                glib::Propagation::Stop
            });
        }
        graph.add_controller(scroll);
    }

    pub fn refresh(&self, app: &App) {
        let m = app.model.borrow();
        let p = &m.program;
        let ch = app.selected.get();
        let s = p.strip(ch);
        for (c, l) in Channel::all().zip(&self.list_labels) {
            let flag = if p.is_muted(c) { "  (muted)" } else { "" };
            l.set_text(&format!("{}  ·  {}{flag}", c.label(), p.channel_name(c)));
        }
        if let Some(r) = self.list.row_at_index(i32::from(ch.index())) {
            if !r.is_selected() {
                self.list.select_row(Some(&r));
            }
        }
        self.title
            .set_text(&format!("{} · {}", ch.label(), p.channel_name(ch)));
        let peq_off = p.peq_bypassed(ch);
        widgets::set_toggle(&self.peq_bypass, peq_off);
        widgets::set_class(&self.peq_bypass, "bypass-on", peq_off);

        let sel = app.selected_band.get();
        for (i, b) in self.bands.iter().enumerate() {
            let band = s.peq.get(i);
            for w in [
                b.select.upcast_ref::<gtk::Widget>(),
                b.kind.upcast_ref(),
                b.freq.upcast_ref(),
                b.gain.upcast_ref(),
                b.q.upcast_ref(),
                b.bypass.upcast_ref(),
            ] {
                w.set_visible(band.is_some());
            }
            let Some(band) = band else { continue };
            widgets::set_toggle(&b.select, i == sel);
            widgets::set_drop(&b.kind, u32::from(band.kind));
            widgets::set_spin(&b.freq, band.freq);
            widgets::set_spin(&b.gain, band.gain);
            widgets::set_spin(&b.q, u16::from(band.q));
            let byp = p.peq_band_bypassed(ch, i as u8);
            if b.bypass.is_active() != byp {
                b.bypass.set_active(byp);
            }
            let shaped = matches!(
                PeqType::from_raw(band.kind),
                Some(PeqType::Peak | PeqType::LowShelf | PeqType::HighShelf)
            );
            b.gain.set_sensitive(shaped);
        }

        // GEQ only exists on inputs.
        self.tabs.page(&self.geq_page).set_visible(ch.is_input());
        if ch.is_input() {
            let input = &p.inputs[ch.index() as usize];
            for ((scale, label), raw) in self.geq.iter().zip(input.geq.iter()) {
                widgets::set_scale(scale, *raw);
                label.set_text(&format!("{:+.1}", units::eq_gain_db(*raw)));
            }
            let off = p.geq_bypassed(ch);
            widgets::set_toggle(&self.geq_bypass, off);
            widgets::set_class(&self.geq_bypass, "bypass-on", off);
        }

        if self.name.text() != p.channel_name(ch) && !self.name.has_focus() {
            self.name.set_text(&p.channel_name(ch));
        }
        widgets::set_spin(&self.gain, s.gain);
        widgets::set_switch(&self.mute, p.is_muted(ch));
        widgets::set_switch(&self.polarity, s.polarity != 0);
        self.delay.update();
        widgets::set_spin(&self.delay, s.delay);
        let linked = p.linked_channels(ch);
        for (other, b) in Channel::all().zip(&self.link_buttons) {
            b.set_visible(other.is_input() == ch.is_input() && other != ch);
            widgets::set_toggle(b, linked.contains(&other));
        }
        for (dst, b) in Channel::all().zip(&self.copy_buttons) {
            b.set_visible(dst.is_input() == ch.is_input() && dst != ch);
        }
        widgets::set_spin(&self.hp_freq, s.high_pass.freq);
        widgets::set_drop(&self.hp_slope, u32::from(s.high_pass.slope));
        widgets::set_spin(&self.lp_freq, s.low_pass.freq);
        widgets::set_drop(&self.lp_slope, u32::from(s.low_pass.slope));

        self.routing.set_visible(!ch.is_input());
        self.comp.set_visible(!ch.is_input());
        self.limiter.set_visible(!ch.is_input());
        self.gate.set_visible(ch.is_input());
        if ch.is_input() {
            let g = p.inputs[ch.index() as usize].gate;
            for (w, v) in self
                .gate_w
                .iter()
                .zip([g.threshold, g.attack, g.hold, g.release])
            {
                widgets::set_spin(w, v);
            }
        } else {
            let o = &p.outputs[ch.index() as usize - 2];
            for (input, (sw, lvl)) in self.route.iter().enumerate() {
                widgets::set_switch(sw, o.routing & (1 << input) != 0);
                widgets::set_spin(lvl, o.mix[input]);
            }
            let c = o.compressor;
            for (w, v) in self
                .comp_w
                .iter()
                .zip([c.threshold, c.attack, c.release, c.knee])
            {
                widgets::set_spin(w, v);
            }
            widgets::set_drop(&self.comp_ratio, u32::from(c.ratio));
            let l = o.limiter;
            for (w, v) in self
                .lim_w
                .iter()
                .zip([l.threshold, l.attack, l.release, l.knee])
            {
                widgets::set_spin(w, v);
            }
        }
        self.graph.queue_draw();
    }

    pub fn redraw_meters(&self) {
        self.meter.queue_draw();
    }
}

/// Add `other` to (or remove it from) `ch`'s link group and send the new
/// masks of every affected channel.
fn set_link(app: &App, ch: Channel, other: Channel, join: bool) {
    if ch.is_input() != other.is_input() || ch == other {
        return;
    }
    let old: Vec<Channel> = app.model.borrow().program.linked_channels(ch);
    let mut group = old.clone();
    if join && !group.contains(&other) {
        group.push(other);
    }
    if !join {
        group.retain(|c| *c != other);
    }
    let mask: u8 = group
        .iter()
        .map(|c| 1u8 << domain_bit(*c))
        .fold(0, |a, b| a | b);
    for c in &group {
        app.edit(Request::SetLink { ch: *c, mask });
    }
    for c in old.iter().chain(std::iter::once(&other)) {
        if !group.contains(c) {
            app.edit(Request::SetLink {
                ch: *c,
                mask: 1 << domain_bit(*c),
            });
        }
    }
}

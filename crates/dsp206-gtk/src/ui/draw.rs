//! Cairo drawing: level meters and the EQ response graph.

use dsp206_proto::units::{self, PeqType};
use dsp206_proto::{Channel, Program};
use gtk::{cairo, gdk};

use crate::response;

pub const GRAPH_DB: f64 = 18.0;
pub const METER_MIN: f64 = -60.0;
pub const METER_MAX: f64 = 18.0;

fn rgba(cr: &cairo::Context, c: &gdk::RGBA, alpha: f32) {
    cr.set_source_rgba(
        c.red().into(),
        c.green().into(),
        c.blue().into(),
        f64::from(c.alpha() * alpha),
    );
}

/// Vertical level meter, `dbu` = None means no data.
pub fn meter(cr: &cairo::Context, w: f64, h: f64, dbu: Option<f64>, fg: &gdk::RGBA) {
    rgba(cr, fg, 0.12);
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.fill();
    let Some(v) = dbu else { return };
    let frac = ((v - METER_MIN) / (METER_MAX - METER_MIN)).clamp(0.0, 1.0);
    let grad = cairo::LinearGradient::new(0.0, h, 0.0, 0.0);
    let stop = |db: f64| (db - METER_MIN) / (METER_MAX - METER_MIN);
    grad.add_color_stop_rgb(0.0, 0.2, 0.82, 0.48);
    grad.add_color_stop_rgb(stop(0.0), 0.2, 0.82, 0.48);
    grad.add_color_stop_rgb(stop(8.0), 0.96, 0.83, 0.18);
    grad.add_color_stop_rgb(stop(15.0), 0.88, 0.11, 0.14);
    grad.add_color_stop_rgb(1.0, 0.88, 0.11, 0.14);
    let _ = cr.set_source(&grad);
    cr.rectangle(0.0, h * (1.0 - frac), w, h * frac);
    let _ = cr.fill();
    // 0 dBu tick
    rgba(cr, fg, 0.5);
    let y0 = h * (1.0 - stop(0.0));
    cr.rectangle(0.0, y0, w, 1.0);
    let _ = cr.fill();
}

/// Geometry shared by drawing and mouse handling.
#[derive(Clone, Copy)]
pub struct Graph {
    pub w: f64,
    pub h: f64,
}

impl Graph {
    pub fn x(&self, f: f64) -> f64 {
        (f / 20.0).log10() / 3.0 * self.w
    }
    pub fn y(&self, db: f64) -> f64 {
        self.h / 2.0 - db / GRAPH_DB * (self.h / 2.0 - 10.0)
    }
    pub fn freq_at(&self, x: f64) -> f64 {
        20.0 * 10f64.powf(3.0 * x / self.w)
    }
    pub fn db_at(&self, y: f64) -> f64 {
        (self.h / 2.0 - y) / (self.h / 2.0 - 10.0) * GRAPH_DB
    }
    /// Handle position of PEQ band `i` of `ch`.
    pub fn handle(&self, p: &Program, ch: Channel, i: usize) -> Option<(f64, f64)> {
        let band = p.strip(ch).peq.get(i)?;
        let f = units::freq_hz(band.freq);
        let gain = match PeqType::from_raw(band.kind) {
            Some(PeqType::Peak | PeqType::LowShelf | PeqType::HighShelf) => {
                units::eq_gain_db(band.gain)
            }
            _ => 0.0,
        };
        Some((self.x(f), self.y(gain)))
    }
}

/// Total response of a channel in dB at `f` (PEQ, GEQ for inputs, crossover).
pub fn total_db(p: &Program, ch: Channel, f: f64) -> f64 {
    let s = p.strip(ch);
    let mut db = response::cut_db(&s.high_pass, true, f) + response::cut_db(&s.low_pass, false, f);
    if !p.peq_bypassed(ch) {
        for (i, band) in s.peq.iter().enumerate() {
            if p.peq_band_bypassed(ch, i as u8) {
                continue;
            }
            if let Some(b) = response::band_response(band) {
                db += b.magnitude_db(f);
            }
        }
    }
    if ch.is_input() && !p.geq_bypassed(ch) {
        let geq = &p.inputs[ch.index() as usize].geq;
        for (i, raw) in geq.iter().enumerate() {
            let g = units::eq_gain_db(*raw);
            if g.abs() > 0.01 {
                db += response::geq_band(units::GEQ_BANDS_HZ[i], g).magnitude_db(f);
            }
        }
    }
    db
}

pub fn eq_graph(
    cr: &cairo::Context,
    g: Graph,
    p: &Program,
    ch: Channel,
    selected: usize,
    fg: &gdk::RGBA,
    accent: &gdk::RGBA,
) {
    rgba(cr, fg, 0.05);
    cr.rectangle(0.0, 0.0, g.w, g.h);
    let _ = cr.fill();
    cr.set_line_width(1.0);
    cr.set_font_size(10.5);
    for f in [
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ] {
        let x = g.x(f).round() + 0.5;
        rgba(cr, fg, 0.10);
        cr.move_to(x, 0.0);
        cr.line_to(x, g.h);
        let _ = cr.stroke();
        rgba(cr, fg, 0.45);
        cr.move_to(x + 3.0, g.h - 5.0);
        let label = if f >= 1000.0 {
            format!("{}k", f / 1000.0)
        } else {
            format!("{f}")
        };
        let _ = cr.show_text(&label);
    }
    for db in [-18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0] {
        let y = g.y(db).round() + 0.5;
        rgba(cr, fg, if db == 0.0 { 0.25 } else { 0.10 });
        cr.move_to(0.0, y);
        cr.line_to(g.w, y);
        let _ = cr.stroke();
        rgba(cr, fg, 0.45);
        cr.move_to(4.0, y - 3.0);
        let _ = cr.show_text(&format!("{db:+.0}"));
    }

    let s = p.strip(ch);
    // Shade the bands removed by the crossover.
    rgba(cr, accent, 0.07);
    let hp = g.x(units::freq_hz(s.high_pass.freq)).max(0.0);
    cr.rectangle(0.0, 0.0, hp, g.h);
    let lp = g.x(units::freq_hz(s.low_pass.freq)).min(g.w);
    cr.rectangle(lp, 0.0, g.w - lp, g.h);
    let _ = cr.fill();

    let axis = response::freq_axis(g.w.max(2.0) as usize);
    // Individual PEQ bands, faint.
    if !p.peq_bypassed(ch) {
        for (i, band) in s.peq.iter().enumerate() {
            if p.peq_band_bypassed(ch, i as u8) {
                continue;
            }
            let Some(b) = response::band_response(band) else {
                continue;
            };
            rgba(cr, fg, if i == selected { 0.45 } else { 0.18 });
            for (k, f) in axis.iter().enumerate() {
                let y = g.y(b.magnitude_db(*f).clamp(-GRAPH_DB - 2.0, GRAPH_DB + 2.0));
                if k == 0 {
                    cr.move_to(g.x(*f), y);
                } else {
                    cr.line_to(g.x(*f), y);
                }
            }
            let _ = cr.stroke();
        }
    }
    // Total response.
    let pts: Vec<(f64, f64)> = axis
        .iter()
        .map(|f| {
            (
                g.x(*f),
                g.y(total_db(p, ch, *f).clamp(-GRAPH_DB - 4.0, GRAPH_DB + 4.0)),
            )
        })
        .collect();
    rgba(cr, accent, 0.18);
    cr.move_to(0.0, g.y(0.0));
    for (x, y) in &pts {
        cr.line_to(*x, *y);
    }
    cr.line_to(g.w, g.y(0.0));
    cr.close_path();
    let _ = cr.fill();
    rgba(cr, accent, 1.0);
    cr.set_line_width(2.5);
    for (k, (x, y)) in pts.iter().enumerate() {
        if k == 0 {
            cr.move_to(*x, *y);
        } else {
            cr.line_to(*x, *y);
        }
    }
    let _ = cr.stroke();

    // Band handles.
    cr.set_font_size(11.0);
    for i in 0..s.peq.len() {
        let Some((x, y)) = g.handle(p, ch, i) else {
            continue;
        };
        let bypassed = p.peq_band_bypassed(ch, i as u8) || p.peq_bypassed(ch);
        let r = if i == selected { 11.0 } else { 9.0 };
        cr.arc(x, y, r, 0.0, std::f64::consts::TAU);
        if bypassed {
            cr.set_source_rgb(0.78, 0.53, 0.0);
        } else if i == selected {
            cr.set_source_rgb(1.0, 1.0, 1.0);
        } else {
            rgba(cr, accent, 1.0);
        }
        let _ = cr.fill();
        if i == selected {
            cr.set_source_rgb(0.1, 0.1, 0.12);
        } else {
            cr.set_source_rgb(1.0, 1.0, 1.0);
        }
        let label = format!("{}", i + 1);
        if let Ok(ext) = cr.text_extents(&label) {
            cr.move_to(
                x - ext.width() / 2.0 - ext.x_bearing(),
                y + ext.height() / 2.0,
            );
        }
        let _ = cr.show_text(&label);
    }
}

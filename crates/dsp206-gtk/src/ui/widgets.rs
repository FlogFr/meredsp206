//! Small widget helpers: spin buttons that edit raw device values but show
//! engineering units.

use adw::prelude::*;
use gtk::glib;

pub fn icon_label(icon: &str, text: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    b.append(&gtk::Image::from_icon_name(icon));
    b.append(&gtk::Label::new(Some(text)));
    b
}

/// A spin button over raw values `min..=max`, displayed with `fmt` and parsed
/// back with `parse` (which returns a raw value).
pub fn raw_spin(
    min: u16,
    max: u16,
    page: u16,
    fmt: impl Fn(u16) -> String + 'static,
    parse: impl Fn(&str) -> Option<u16> + 'static,
) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(f64::from(min), f64::from(max), 1.0);
    spin.set_increments(1.0, f64::from(page));
    spin.set_digits(0);
    spin.set_numeric(false);
    spin.set_width_chars(10);
    spin.set_valign(gtk::Align::Center);
    spin.add_css_class("value");
    spin.connect_output(move |s| {
        s.set_text(&fmt(s.value().round() as u16));
        glib::Propagation::Stop
    });
    spin.connect_input(move |s| parse(&s.text()).map(|v| Ok(f64::from(v))));
    spin
}

pub fn row(title: &str, subtitle: Option<&str>, suffix: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    let r = adw::ActionRow::builder().title(title).build();
    if let Some(s) = subtitle {
        r.set_subtitle(s);
    }
    r.add_suffix(suffix);
    r
}

pub fn dropdown(items: &[&str]) -> gtk::DropDown {
    let d = gtk::DropDown::from_strings(items);
    d.set_valign(gtk::Align::Center);
    d
}

pub fn switch() -> gtk::Switch {
    let s = gtk::Switch::new();
    s.set_valign(gtk::Align::Center);
    s
}

/// Flip a style class on/off.
pub fn set_class(w: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        w.add_css_class(class);
    } else {
        w.remove_css_class(class);
    }
}

/// Set a spin button value without emitting when unchanged.
pub fn set_spin(s: &gtk::SpinButton, v: u16) {
    if (s.value() - f64::from(v)).abs() > 0.1 {
        s.set_value(f64::from(v));
    }
}

pub fn set_scale(s: &gtk::Scale, v: u16) {
    if (s.value() - f64::from(v)).abs() > 0.1 {
        s.set_value(f64::from(v));
    }
}

pub fn set_drop(d: &gtk::DropDown, v: u32) {
    if d.selected() != v {
        d.set_selected(v);
    }
}

pub fn set_toggle(t: &impl IsA<gtk::ToggleButton>, on: bool) {
    if t.is_active() != on {
        t.set_active(on);
    }
}

pub fn set_switch(s: &gtk::Switch, on: bool) {
    if s.is_active() != on {
        s.set_active(on);
    }
}

//! Protocol log: every frame exchanged with the device, for bug reports.

use std::rc::Rc;

use adw::prelude::*;

use crate::app::App;

pub struct LogPage {
    pub root: gtk::Box,
    view: gtk::TextView,
    follow: gtk::CheckButton,
    hide_meters: gtk::CheckButton,
}

impl LogPage {
    pub fn build(app: &Rc<App>) -> LogPage {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_margin_top(8);
        root.set_margin_bottom(8);
        root.set_margin_start(8);
        root.set_margin_end(8);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let follow = gtk::CheckButton::with_label("Follow");
        follow.set_active(true);
        let hide_meters = gtk::CheckButton::with_label("Hide meter polling");
        hide_meters.set_active(true);
        let copy = gtk::Button::with_label("Copy all");
        let clear = gtk::Button::with_label("Clear");
        let hint = gtk::Label::new(Some("Attach this log when reporting a problem."));
        hint.add_css_class("dim-label");
        hint.set_hexpand(true);
        hint.set_xalign(0.0);
        bar.append(&hint);
        bar.append(&hide_meters);
        bar.append(&follow);
        bar.append(&copy);
        bar.append(&clear);
        root.append(&bar);
        let view = gtk::TextView::builder()
            .editable(false)
            .monospace(true)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .build();
        root.append(&scroll);
        {
            let app = Rc::clone(app);
            clear.connect_clicked(move |_| {
                app.log.borrow_mut().clear();
                app.log_dirty.set(true);
            });
        }
        {
            let view = view.clone();
            copy.connect_clicked(move |b| {
                let buf = view.buffer();
                let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
                b.clipboard().set_text(&text);
            });
        }
        {
            let app = Rc::clone(app);
            hide_meters.connect_toggled(move |_| app.log_dirty.set(true));
        }
        LogPage {
            root,
            view,
            follow,
            hide_meters,
        }
    }

    pub fn refresh(&self, app: &App) {
        if !self.root.is_mapped() {
            app.log_dirty.set(true);
            return;
        }
        let hide = self.hide_meters.is_active();
        let log = app.log.borrow();
        let joined = log
            .iter()
            .filter(|l| !(hide && l.ends_with("meters")))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        let buf = self.view.buffer();
        buf.set_text(&joined);
        if self.follow.is_active() {
            let mut end = buf.end_iter();
            self.view.scroll_to_iter(&mut end, 0.0, false, 0.0, 1.0);
        }
    }
}

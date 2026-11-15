//! meredsp206: GTK 4 / libadwaita editor for the t.racks DSP 206.

mod app;
mod format;
mod model;
mod response;
mod ui;

const USAGE: &str = "usage: meredsp206 [--demo | --usb | --connect IP] [--page mixer|channel|device|log] [--channel ina|inb|out1..out6]";

fn main() -> gtk::glib::ExitCode {
    let mut opts = app::Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--demo" => opts.demo = true,
            "--usb" => opts.usb = true,
            "--connect" => opts.connect = args.next(),
            "--page" => opts.page = args.next(),
            "--channel" => opts.channel = args.next(),
            "-h" | "--help" => {
                println!("{USAGE}");
                return gtk::glib::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown option {other}\n{USAGE}");
                return gtk::glib::ExitCode::FAILURE;
            }
        }
    }
    app::run(opts)
}

//! `dsp206`: command-line access to a t.racks DSP 206.

mod args;

use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use args::{parse, Cli, Command};
use dsp206_link::{discover, find_usb_devices, Event, Link, LinkConfig, MockDevice, MockOptions};
use dsp206_proto::units::{delay_ms, gain_db};
use dsp206_proto::{Channel, Program, Request};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse(&argv) {
        Ok(cli) => match run(cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{e}\n\n{}", args::USAGE);
            ExitCode::from(2)
        }
    }
}

fn connect(cli: &Cli) -> Result<Link, String> {
    let cfg = LinkConfig {
        device_id: cli.id,
        ..LinkConfig::default()
    };
    if cli.usb {
        let path = match &cli.hidraw {
            Some(p) => p.clone(),
            None => {
                find_usb_devices()
                    .map_err(|e| format!("cannot list USB devices: {e}"))?
                    .into_iter()
                    .next()
                    .ok_or("no DSP 206 found on USB")?
                    .path
            }
        };
        return Link::open_usb(&path, cfg).map_err(|e| format!("cannot open USB: {e}"));
    }
    let addr = SocketAddr::new(cli.host, cli.port);
    Link::connect(addr, cfg).map_err(|e| format!("cannot connect to {addr}: {e}"))
}

/// Drain events until `done` or timeout, printing traces when asked.
fn pump(
    link: &Link,
    cli: &Cli,
    timeout: Duration,
    mut done: impl FnMut(&Event) -> bool,
) -> Vec<Event> {
    let end = Instant::now() + timeout;
    let mut out = Vec::new();
    while Instant::now() < end {
        let Some(ev) = link.recv_timeout(Duration::from_millis(20)) else {
            if link.pending() == 0 && !out.is_empty() && timeout < Duration::from_secs(1000) {
                // Nothing left to do.
            }
            continue;
        };
        if let Event::Trace { outgoing, bytes } = &ev {
            if cli.trace {
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                eprintln!("{} {}", if *outgoing { "->" } else { "<-" }, hex.join(" "));
            }
            continue;
        }
        let stop = done(&ev);
        out.push(ev);
        if stop {
            break;
        }
    }
    out
}

fn print_program(p: &Program) {
    println!("Program: {}", p.name_text());
    for ch in Channel::all() {
        let s = p.strip(ch);
        println!(
            "  {:<5} {:<8} gain {:+6.1} dB  delay {:7.2} ms  {}{}",
            ch.label(),
            p.channel_name(ch),
            gain_db(s.gain),
            delay_ms(s.delay),
            if p.is_muted(ch) { "MUTED " } else { "" },
            if s.polarity != 0 { "INVERTED" } else { "" },
        );
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match &cli.command {
        Command::Help => println!("{}", args::USAGE),
        Command::Discover => {
            match find_usb_devices() {
                Ok(list) if list.is_empty() => println!("No DSP 206 on USB."),
                Ok(list) => {
                    for d in list {
                        println!("{}  {}  {}", d.path.display(), d.name, d.phys);
                    }
                }
                Err(e) => println!("Cannot list USB devices: {e}"),
            }
            let extra: Vec<IpAddr> = vec![cli.host];
            let found = discover(Duration::from_secs(2), &extra).map_err(|e| e.to_string())?;
            if found.is_empty() {
                println!("No device answered on the network.");
            }
            for d in found {
                println!("{}  {}  {}", d.addr, d.name, d.details.join(" "));
            }
        }
        Command::Info | Command::Dump(_) => {
            let link = connect(&cli)?;
            link.read_all();
            let events = pump(&link, &cli, Duration::from_secs(60), |e| {
                matches!(e, Event::PresetSlots(_) | Event::Closed(_))
            });
            let mut program = None;
            for ev in &events {
                match ev {
                    Event::Identity(id) => println!("Device: {id}"),
                    Event::CurrentPreset(n) => println!("Current preset: {n}"),
                    Event::PresetName { index, name } if cli.verbose => {
                        println!("  U{index:02}: {name}")
                    }
                    Event::Program(p) => program = Some(p.clone()),
                    Event::ProgramIncomplete(m) => {
                        println!("Program incomplete, missing pages {m:?}")
                    }
                    Event::Health(h) => println!("Link: {h:?}"),
                    Event::Timeout(r) if cli.verbose => println!("No reply to {r:?}"),
                    _ => {}
                }
            }
            if let Some(p) = &program {
                print_program(p);
                if let Command::Dump(Some(path)) = &cli.command {
                    std::fs::write(path, p.to_file()).map_err(|e| format!("{path}: {e}"))?;
                    println!("Saved {path}");
                }
            }
            link.close();
            if program.is_none() {
                return Err("the program could not be read completely".into());
            }
        }
        Command::Meters(count) => {
            let link = connect(&cli)?;
            link.send(Request::Connect);
            link.set_metering(true);
            let mut n = 0;
            pump(&link, &cli, Duration::from_secs(3600), |e| {
                if let Event::Meters(m) = e {
                    let line: Vec<String> = m
                        .inputs
                        .iter()
                        .chain(m.outputs.iter())
                        .map(|v| format!("{v:6.1}"))
                        .collect();
                    println!("InA InB Out1-6 (dBu): {}", line.join(" "));
                    n += 1;
                }
                n >= *count || matches!(e, Event::Closed(_))
            });
            link.close();
        }
        Command::Set(requests) => {
            let link = connect(&cli)?;
            link.send(Request::Connect);
            for r in requests {
                link.send(r.clone());
            }
            let events = pump(&link, &cli, Duration::from_secs(10), |_| false);
            for ev in events {
                if let Event::Unconfirmed(r) = ev {
                    println!("Sent without acknowledgement: {r:?}");
                }
            }
            link.close();
        }
        Command::Load(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let p = Program::from_file(&bytes).map_err(|e| format!("{path}: {e}"))?;
            let link = connect(&cli)?;
            link.send(Request::Connect);
            let reqs = p.sync_requests();
            println!("Sending {} settings…", reqs.len());
            for r in reqs {
                link.send(r);
            }
            while link.pending() > 0 {
                pump(&link, &cli, Duration::from_millis(200), |_| false);
            }
            link.close();
        }
        Command::Mock => {
            let addr = SocketAddr::new(cli.host, cli.port);
            let mock = MockDevice::start_on(addr, MockOptions::default())
                .map_err(|e| format!("{addr}: {e}"))?;
            println!(
                "Simulated DSP 206 listening on {} (Ctrl+C to stop)",
                mock.addr()
            );
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
    }
    Ok(())
}

//! Argument parsing (no dependency).

use std::net::IpAddr;
use std::path::PathBuf;

use dsp206_proto::program::name_field;
use dsp206_proto::units::{delay_raw, gain_raw};
use dsp206_proto::{Channel, Request, ToneSource};

pub const USAGE: &str = "\
usage: dsp206 [--host IP] [--port N] [--usb] [--hidraw PATH] [--id N] [--trace] [-v] COMMAND

Commands:
  discover                 find DSP 206 units on USB and on the LAN (UDP 30303)
  info                     read identity, presets and the current program
  dump [FILE]              read the current program and save it as a .dsp206 file
  meters [COUNT]           print level meters (default 20 readings)
  load FILE                send a .dsp206 program file to the device
  set SETTING...           change settings on the device:
      gain CH DB           channel gain in dB (-60..+12)
      mute CH on|off
      polarity CH on|off
      delay CH MS          delay in ms (0..680)
      name CH TEXT         channel name (8 characters)
      recall N             recall preset N
      tone analog|pink|white|sine [BAND]   test tone, BAND 0..30 (20 Hz..20 kHz)
  mock                     run a simulated device on --host/--port (default 127.0.0.1:9761)
  help

CH is ina, inb, out1..out6 (or 0..7). Defaults: --host 192.168.1.101 --port 9761 --id 1.
--usb talks to the unit plugged in over USB instead (--hidraw picks the node,
e.g. /dev/hidraw5). Over the network, wait about one minute between two
connections to a real unit.";

#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    Discover,
    Info,
    Dump(Option<String>),
    Meters(usize),
    Load(String),
    Set(Vec<Request>),
    Mock,
}

#[derive(Debug)]
pub struct Cli {
    pub host: IpAddr,
    pub port: u16,
    /// Use USB instead of the network.
    pub usb: bool,
    /// USB node to open; found automatically when `None`.
    pub hidraw: Option<PathBuf>,
    pub id: u8,
    pub trace: bool,
    pub verbose: bool,
    pub command: Command,
}

pub fn parse_channel(s: &str) -> Result<Channel, String> {
    let lower = s.to_ascii_lowercase();
    let ch = match lower.as_str() {
        "ina" | "a" => Channel::new(0),
        "inb" | "b" => Channel::new(1),
        _ => match lower.strip_prefix("out") {
            Some(n) => n.parse::<u8>().ok().and_then(Channel::output),
            None => lower.parse::<u8>().ok().and_then(Channel::new),
        },
    };
    ch.ok_or_else(|| format!("unknown channel '{s}'"))
}

fn on_off(s: &str) -> Result<bool, String> {
    match s {
        "on" | "1" | "true" | "yes" => Ok(true),
        "off" | "0" | "false" | "no" => Ok(false),
        _ => Err(format!("expected on/off, got '{s}'")),
    }
}

fn number<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    s.parse().map_err(|_| format!("invalid {what} '{s}'"))
}

fn parse_settings(words: &[String]) -> Result<Vec<Request>, String> {
    let mut out = Vec::new();
    let mut it = words.iter();
    let mut next = |what: &str| it.next().cloned().ok_or_else(|| format!("missing {what}"));
    loop {
        let Ok(setting) = next("setting") else { break };
        let req = match setting.as_str() {
            "gain" => {
                let ch = parse_channel(&next("channel")?)?;
                let db: f64 = number(&next("dB")?, "gain")?;
                Request::SetGain {
                    ch,
                    raw: gain_raw(db),
                }
            }
            "mute" => Request::SetMute {
                ch: parse_channel(&next("channel")?)?,
                on: on_off(&next("on/off")?)?,
            },
            "polarity" => Request::SetPolarity {
                ch: parse_channel(&next("channel")?)?,
                inverted: on_off(&next("on/off")?)?,
            },
            "delay" => {
                let ch = parse_channel(&next("channel")?)?;
                let ms: f64 = number(&next("ms")?, "delay")?;
                Request::SetDelay {
                    ch,
                    raw: delay_raw(ms),
                }
            }
            "name" => Request::SetChannelName {
                ch: parse_channel(&next("channel")?)?,
                name: name_field(&next("name")?),
            },
            "recall" => Request::RecallPreset(number(&next("preset")?, "preset")?),
            "tone" => {
                let source = match next("source")?.as_str() {
                    "analog" => ToneSource::AnalogInput,
                    "pink" => ToneSource::PinkNoise,
                    "white" => ToneSource::WhiteNoise,
                    "sine" => ToneSource::Sine,
                    s => return Err(format!("unknown tone source '{s}'")),
                };
                let freq = if source == ToneSource::Sine {
                    number::<u8>(&next("band")?, "band").and_then(|b| {
                        if b <= 30 {
                            Ok(b)
                        } else {
                            Err("band must be 0..30".into())
                        }
                    })?
                } else {
                    0
                };
                Request::TestTone {
                    source: source as u8,
                    freq,
                }
            }
            other => return Err(format!("unknown setting '{other}'")),
        };
        out.push(req);
    }
    if out.is_empty() {
        return Err("set needs at least one setting".into());
    }
    Ok(out)
}

pub fn parse(argv: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        host: [192, 168, 1, 101].into(),
        port: dsp206_link::PORT,
        usb: false,
        hidraw: None,
        id: 1,
        trace: false,
        verbose: false,
        command: Command::Help,
    };
    let mut host_given = false;
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].as_str();
        let mut value = || {
            i += 1;
            argv.get(i)
                .cloned()
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a {
            "--host" => {
                cli.host = number(&value()?, "host")?;
                host_given = true;
            }
            "--port" => cli.port = number(&value()?, "port")?,
            "--usb" => cli.usb = true,
            "--hidraw" => {
                cli.hidraw = Some(value()?.into());
                cli.usb = true;
            }
            "--id" => cli.id = number(&value()?, "device id")?,
            "--trace" => cli.trace = true,
            "-v" | "--verbose" => cli.verbose = true,
            "-h" | "--help" => return Ok(cli),
            _ => break,
        }
        i += 1;
    }
    let rest = &argv[i..];
    cli.command = match rest.first().map(String::as_str) {
        None | Some("help") => Command::Help,
        Some("discover") => Command::Discover,
        Some("info") => Command::Info,
        Some("dump") => Command::Dump(rest.get(1).cloned()),
        Some("meters") => Command::Meters(match rest.get(1) {
            Some(n) => number(n, "count")?,
            None => 20,
        }),
        Some("load") => Command::Load(rest.get(1).cloned().ok_or("load needs a file")?),
        Some("set") => Command::Set(parse_settings(&rest[1..])?),
        Some("mock") => {
            if !host_given {
                cli.host = [127, 0, 0, 1].into();
            }
            Command::Mock
        }
        Some(other) => return Err(format!("unknown command '{other}'")),
    };
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn channels() {
        assert_eq!(parse_channel("InA"), Ok(Channel::IN_A));
        assert_eq!(parse_channel("out6"), Ok(Channel::new(7).unwrap()));
        assert_eq!(parse_channel("3"), Ok(Channel::new(3).unwrap()));
        assert!(parse_channel("out7").is_err());
        assert!(parse_channel("x").is_err());
    }

    #[test]
    fn global_options_and_commands() {
        let cli = parse(&args("--host 10.0.0.5 --id 3 --trace dump prog.dsp206")).unwrap();
        assert_eq!(cli.host, IpAddr::from([10, 0, 0, 5]));
        assert_eq!((cli.id, cli.trace), (3, true));
        assert_eq!(cli.command, Command::Dump(Some("prog.dsp206".into())));
        assert_eq!(
            parse(&args("meters 5")).unwrap().command,
            Command::Meters(5)
        );
        assert_eq!(parse(&[]).unwrap().command, Command::Help);
        assert!(!parse(&args("info")).unwrap().usb);
        assert!(parse(&args("frobnicate")).is_err());
        // mock listens on localhost unless told otherwise
        assert_eq!(
            parse(&args("mock")).unwrap().host,
            IpAddr::from([127, 0, 0, 1])
        );
    }

    #[test]
    fn usb_options() {
        let cli = parse(&args("--usb info")).unwrap();
        assert!(cli.usb);
        assert_eq!((cli.hidraw, cli.command), (None, Command::Info));
        let cli = parse(&args("--hidraw /dev/hidraw5 meters 3")).unwrap();
        assert!(cli.usb);
        assert_eq!(cli.hidraw, Some(PathBuf::from("/dev/hidraw5")));
        assert_eq!(cli.command, Command::Meters(3));
        assert!(parse(&args("--hidraw")).is_err());
    }

    #[test]
    fn settings() {
        let cli = parse(&args(
            "set gain out1 -3 mute ina on delay out2 1 tone sine 17",
        ))
        .unwrap();
        let Command::Set(reqs) = cli.command else {
            panic!()
        };
        assert_eq!(
            reqs,
            vec![
                Request::SetGain {
                    ch: Channel::output(1).unwrap(),
                    raw: 250
                },
                Request::SetMute {
                    ch: Channel::IN_A,
                    on: true
                },
                Request::SetDelay {
                    ch: Channel::output(2).unwrap(),
                    raw: 96
                },
                Request::TestTone {
                    source: 3,
                    freq: 17
                },
            ]
        );
        assert!(parse(&args("set gain out1")).is_err());
        assert!(parse(&args("set tone sine 31")).is_err());
        assert!(parse(&args("set")).is_err());
    }
}

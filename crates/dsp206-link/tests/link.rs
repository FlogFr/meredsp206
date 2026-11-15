//! Link worker against the simulated device (never the real one).

#![allow(clippy::expect_used, clippy::unwrap_used)] // tests may panic

use std::time::{Duration, Instant};

use dsp206_link::{Event, Health, Link, LinkConfig, MockDevice, MockOptions};
use dsp206_proto::program::PAGE_COUNT;
use dsp206_proto::{Channel, Program, Request};

fn fast() -> LinkConfig {
    LinkConfig {
        read_timeout: Duration::from_millis(150),
        write_timeout: Duration::from_millis(150),
        meter_interval: Duration::from_millis(20),
        ..LinkConfig::default()
    }
}

/// Collect events until `stop` returns true or the timeout expires.
fn collect(link: &Link, timeout: Duration, mut stop: impl FnMut(&Event) -> bool) -> Vec<Event> {
    let end = Instant::now() + timeout;
    let mut out = Vec::new();
    while Instant::now() < end {
        if let Some(ev) = link.recv_timeout(Duration::from_millis(10)) {
            let done = stop(&ev);
            out.push(ev);
            if done {
                break;
            }
        }
    }
    out
}

fn wait_idle(link: &Link) {
    let end = Instant::now() + Duration::from_secs(5);
    while link.pending() > 0 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(30));
}

#[test]
fn reads_everything_from_device() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    mock.state().presets[3].name = dsp206_link::mock::preset_name("Main PA");
    let link = Link::connect(mock.addr(), fast()).unwrap();
    link.read_all();
    let events = collect(&link, Duration::from_secs(5), |e| {
        matches!(e, Event::PresetSlots(_))
    });
    assert!(events.contains(&Event::Handshake(0x19)));
    assert!(events.contains(&Event::Identity("DSP206 V0105P".into())));
    assert!(events.contains(&Event::Program(Program::factory_default())));
    assert!(events.contains(&Event::CurrentPreset(0)));
    assert!(events.contains(&Event::PresetName {
        index: 3,
        name: "Main PA".into()
    }));
    let names = events
        .iter()
        .filter(|e| matches!(e, Event::PresetName { .. }))
        .count();
    assert_eq!(names, 21);
    let pages = events
        .iter()
        .filter(|e| matches!(e, Event::Page { .. }))
        .count();
    assert_eq!(pages, PAGE_COUNT);
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::Timeout(_) | Event::ProgramIncomplete(_))));
    // The program is read before the preset names (the device may go quiet).
    let pos = |f: &dyn Fn(&Event) -> bool| events.iter().position(f).unwrap();
    assert!(
        pos(&|e| matches!(e, Event::Program(_))) < pos(&|e| matches!(e, Event::PresetName { .. }))
    );
}

#[test]
fn setters_reach_the_device_and_are_acknowledged() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    let out3 = Channel::output(3).unwrap();
    link.send(Request::SetGain { ch: out3, raw: 200 });
    link.send(Request::SetMute { ch: out3, on: true });
    wait_idle(&link);
    let st = mock.state();
    assert_eq!(st.program.outputs[2].gain, 200);
    assert!(st.program.is_muted(out3));
    drop(st);
    let events = collect(&link, Duration::from_millis(100), |_| false);
    assert!(!events.iter().any(|e| matches!(e, Event::Unconfirmed(_))));
}

#[test]
fn queued_setters_are_coalesced() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    let ch = Channel::IN_A;
    for raw in 0..=200 {
        link.send(Request::SetGain { ch, raw });
    }
    wait_idle(&link);
    let st = mock.state();
    let gains: Vec<u16> = st
        .received
        .iter()
        .filter_map(|r| match r {
            Request::SetGain { raw, .. } => Some(*raw),
            _ => None,
        })
        .collect();
    assert!(
        gains.len() < 50,
        "{} setters reached the device",
        gains.len()
    );
    assert_eq!(gains.last(), Some(&200));
    assert_eq!(st.program.inputs[0].gain, 200);
}

#[test]
fn silent_device_is_detected_and_missing_pages_retried() {
    // Like the real unit: answers a burst, then stops.
    let mock = MockDevice::start(MockOptions {
        reply_limit: Some(10),
        ..MockOptions::default()
    })
    .unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    link.read_program();
    let events = collect(&link, Duration::from_secs(20), |e| {
        matches!(e, Event::ProgramIncomplete(_))
    });
    assert!(events.contains(&Event::Health(Health::Silent)));
    let Some(Event::ProgramIncomplete(missing)) = events.last() else {
        panic!("no ProgramIncomplete in {events:?}");
    };
    assert_eq!(missing, &(10..PAGE_COUNT as u8).collect::<Vec<_>>());
    // Device answers again: only the missing pages are requested.
    mock.state().replies_sent = 0;
    mock.state().received.clear();
    link.retry_missing_pages();
    let events = collect(&link, Duration::from_secs(5), |e| {
        matches!(e, Event::Program(_))
    });
    assert!(events.contains(&Event::Health(Health::Answering)));
    assert!(matches!(events.last(), Some(Event::Program(_))));
    let asked: Vec<Request> = mock
        .state()
        .received
        .iter()
        .filter(|r| matches!(r, Request::ReadProgramPage(_)))
        .cloned()
        .collect();
    assert_eq!(asked.len(), PAGE_COUNT - 10);
}

#[test]
fn unacknowledged_setter_is_reported() {
    let mock = MockDevice::start(MockOptions {
        silent_writes: true,
        ..MockOptions::default()
    })
    .unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    let req = Request::SetPolarity {
        ch: Channel::IN_B,
        inverted: true,
    };
    link.send(req.clone());
    let events = collect(&link, Duration::from_secs(2), |e| {
        matches!(e, Event::Unconfirmed(_))
    });
    assert_eq!(events.last(), Some(&Event::Unconfirmed(req)));
    // The setter still reached the device.
    assert_eq!(mock.state().program.inputs[1].polarity, 1);
}

#[test]
fn metering_and_trace() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    link.set_metering(true);
    let events = collect(&link, Duration::from_secs(2), |e| {
        matches!(e, Event::Meters(_))
    });
    let Some(Event::Meters(m)) = events.last() else {
        panic!("no meters in {events:?}");
    };
    assert!(m.inputs.iter().all(|db| (-40.0..10.0).contains(db)));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Trace { outgoing: true, bytes } if bytes[5] == 0x40)));
    link.set_metering(false);
}

#[test]
fn recall_then_fresh_read() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    mock.state().presets[2].inputs[0].gain = 123;
    let link = Link::connect(mock.addr(), fast()).unwrap();
    link.send(Request::RecallPreset(2));
    link.read_program();
    let events = collect(&link, Duration::from_secs(5), |e| {
        matches!(e, Event::Program(_))
    });
    let Some(Event::Program(p)) = events.last() else {
        panic!("no program");
    };
    assert_eq!(p.inputs[0].gain, 123);
    assert_eq!(mock.state().current_preset, 2);
}

#[test]
fn close_sends_disconnect() {
    let mock = MockDevice::start(MockOptions::default()).unwrap();
    let link = Link::connect(mock.addr(), fast()).unwrap();
    link.send(Request::Identify);
    wait_idle(&link);
    link.close();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(mock.state().received.last(), Some(&Request::Disconnect));
}

//! Replays frames captured on a real DSP 206 (tools/re/captures/*.json).
//! These are the reference vectors: if a change breaks them, the change is wrong
//! or the capture must be re-checked on hardware.

#![allow(clippy::expect_used, clippy::unwrap_used)] // tests may panic

use dsp206_proto::command::{Reply, READ_ONLY_CODES};
use dsp206_proto::program::{name_text, Program, ProgramAssembler};
use dsp206_proto::units::{eq_gain_db, gain_db, q_value, threshold_db, time_ms};
use dsp206_proto::{hid, Frame, FrameDecoder};

const CAPTURES: [(&str, &str); 2] = [
    (
        "default-program-partial",
        include_str!("../../../tools/re/captures/default-program-partial.json"),
    ),
    (
        "dump-run12",
        include_str!("../../../tools/re/captures/dump-run12.json"),
    ),
];

/// USB sessions (protocol.md §1.2). Kept apart from `CAPTURES`: they include
/// `0x12` and the user-approved Out6 mute, so they are not read-only.
const USB_FULL_READ: &str = include_str!("../../../tools/re/captures/usb-01-full-read.json");
const USB_METERS: &str = include_str!("../../../tools/re/captures/usb-02-meters-30s.json");
const USB_MUTE: &str = include_str!("../../../tools/re/captures/usb-03-mute-out6.json");
/// Read-only, after a "store U01" that sent `0x20 01` (a load) by mistake.
const USB_AFTER_STORE: &str =
    include_str!("../../../tools/re/captures/usb-04-after-store-attempt.json");
/// Read-only, after stores into U01, U02, U19 and U20.
const USB_STORED: &str = include_str!("../../../tools/re/captures/usb-05-stored-presets.json");
const USB_CURRENT_NAME: &str = include_str!("../../../tools/re/captures/usb-06-current-name.json");

/// `(direction, bytes)` of every frame, without a JSON dependency.
fn frames(json: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut rest = json;
    while let Some(i) = rest.find("\"dir\": \"") {
        rest = &rest[i + 8..];
        let dir = rest[..2].to_string();
        let j = rest.find("\"hex\": \"").expect("frame without hex");
        rest = &rest[j + 8..];
        let end = rest.find('"').expect("unterminated hex");
        let bytes = (0..end)
            .step_by(2)
            .map(|k| u8::from_str_radix(&rest[k..k + 2], 16).expect("bad hex"))
            .collect();
        out.push((dir, bytes));
    }
    out
}

#[test]
fn every_captured_frame_decodes() {
    for (name, json) in CAPTURES {
        let all = frames(json);
        assert!(all.len() > 50, "{name}: too few frames parsed");
        for (dir, bytes) in all {
            let f = Frame::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}: {bytes:02x?}"));
            assert_eq!(f.encode().unwrap(), bytes, "{name}: re-encoding differs");
            match dir.as_str() {
                "tx" => assert_eq!((f.src, f.dst), (0, 1), "{name}"),
                "rx" => assert_eq!((f.src, f.dst), (1, 0), "{name}"),
                other => panic!("{name}: unknown direction {other}"),
            }
        }
    }
}

#[test]
fn captures_only_contain_read_only_requests() {
    // Safety net: nothing that changes the sound was ever sent while probing.
    for (name, json) in CAPTURES {
        for (dir, bytes) in frames(json) {
            if dir == "tx" {
                assert!(
                    READ_ONLY_CODES.contains(&bytes[5]),
                    "{name}: write command {:#04x}",
                    bytes[5]
                );
            }
        }
    }
}

#[test]
fn stream_decoder_reads_concatenated_replies() {
    let rx: Vec<u8> = frames(CAPTURES[0].1)
        .into_iter()
        .filter(|(d, _)| d == "rx")
        .flat_map(|(_, b)| b)
        .collect();
    let mut dec = FrameDecoder::new();
    let mut n = 0;
    for chunk in rx.chunks(7) {
        dec.push(chunk);
        for f in dec.by_ref() {
            f.unwrap();
            n += 1;
        }
    }
    assert_eq!(
        n,
        frames(CAPTURES[0].1)
            .iter()
            .filter(|(d, _)| d == "rx")
            .count()
    );
}

#[test]
fn identity_and_preset_names() {
    let mut names = 0;
    for (dir, bytes) in frames(CAPTURES[0].1) {
        if dir != "rx" {
            continue;
        }
        match Reply::parse(&Frame::decode(&bytes).unwrap()) {
            Reply::Identity(id) => assert_eq!(id, "DSP206 V0105P"),
            Reply::Connected { status } => assert_eq!(status, 0x19),
            Reply::PresetName { name, .. } => {
                assert_eq!(name, "Default Preset");
                names += 1;
            }
            _ => {}
        }
    }
    assert_eq!(names, 21);
}

/// Factory "Default Preset" values (memory-map.md §5), on the captured pages.
#[test]
fn captured_pages_match_factory_defaults() {
    let mut asm = ProgramAssembler::new();
    for (dir, bytes) in frames(CAPTURES[0].1) {
        if dir == "rx" {
            if let Reply::ProgramPage { page, data } = Reply::parse(&Frame::decode(&bytes).unwrap())
            {
                asm.insert(page, &data).unwrap();
            }
        }
    }
    assert_eq!(asm.missing_pages(), vec![8, 9, 11, 12, 13, 14, 16, 17, 18]);
    let p = Program::from_bytes(asm.raw()).unwrap();

    assert_eq!(p.header, [0xff, 0x00]);
    assert_eq!(p.name_text(), "Default Preset");
    for (k, ch) in p.inputs.iter().enumerate() {
        assert_eq!(name_text(&ch.name), ["InA", "InB"][k]);
        assert_eq!(gain_db(ch.gain), 0.0);
        assert_eq!((ch.polarity, ch.delay), (0, 0));
        assert_eq!(ch.link, 1 << k);
        assert!(ch.geq.iter().all(|g| eq_gain_db(*g) == 0.0));
        let freqs: Vec<u16> = ch.peq.iter().map(|b| b.freq).collect();
        assert_eq!(freqs, [41, 71, 101, 140, 170, 200, 240, 270]);
        assert!(ch
            .peq
            .iter()
            .all(|b| b.gain == 120 && q_value(b.q) == Some(3.0) && b.kind == 0));
        assert_eq!((ch.high_pass.freq, ch.low_pass.freq), (0, 300));
        assert_eq!((ch.gate.attack, ch.gate.hold, ch.gate.release), (0, 99, 99));
        assert_eq!(threshold_db(ch.gate.threshold), -90.0);
    }

    let out1 = &p.outputs[0];
    assert_eq!(name_text(&out1.name), "Out1");
    assert_eq!((out1.routing, out1.mix), (1, [280, 280]));
    let freqs: Vec<u16> = out1.peq.iter().map(|b| b.freq).collect();
    assert_eq!(freqs, [31, 63, 95, 127, 158, 190, 222, 253, 285]);
    let c = out1.compressor;
    assert_eq!(
        (c.ratio, time_ms(c.attack), time_ms(c.release), c.knee),
        (0, 50, 500, 0)
    );
    assert_eq!(threshold_db(c.threshold), 20.0);
    let l = out1.limiter;
    assert_eq!(
        (
            time_ms(l.attack),
            time_ms(l.release),
            l.knee,
            threshold_db(l.threshold)
        ),
        (50, 500, 0, 20.0)
    );
    assert_eq!(
        (gain_db(out1.gain), out1.polarity, out1.delay, out1.link),
        (0.0, 0, 0, 1)
    );

    // Out2 name starts at the end of page 7.
    assert_eq!(&p.outputs[1].name[..4], b"Out2");
    // Out5 tail (page 15) and Out6 name.
    assert_eq!((p.outputs[4].gain, p.outputs[4].link), (280, 1 << 4));
    assert_eq!(&p.outputs[5].name[..4], b"Out6");
}

#[test]
fn factory_default_matches_every_captured_page() {
    let factory = Program::factory_default().to_pages();
    let mut checked = 0;
    for (dir, bytes) in frames(CAPTURES[0].1) {
        if dir != "rx" {
            continue;
        }
        if let Reply::ProgramPage { page, data } = Reply::parse(&Frame::decode(&bytes).unwrap()) {
            assert_eq!(
                &factory[page as usize][..data.len()],
                &data[..],
                "page {page}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 10);
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|k| u8::from_str_radix(&s[k..k + 2], 16).expect("bad hex"))
        .collect()
}

/// Value of a string field in a one-object-per-line capture entry.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(&format!("\"{key}\": \""))? + key.len() + 5;
    let len = line[start..].find('"')?;
    Some(&line[start..start + len])
}

fn program_from(json: &str) -> ProgramAssembler {
    let mut asm = ProgramAssembler::new();
    for (dir, bytes) in frames(json) {
        if dir == "rx" {
            if let Reply::ProgramPage { page, data } = Reply::parse(&Frame::decode(&bytes).unwrap())
            {
                asm.insert(page, &data).unwrap();
            }
        }
    }
    asm
}

#[test]
fn usb_sessions_are_well_formed_and_fully_answered() {
    for (name, json) in [
        ("usb-01", USB_FULL_READ),
        ("usb-02", USB_METERS),
        ("usb-03", USB_MUTE),
        ("usb-04", USB_AFTER_STORE),
        ("usb-05", USB_STORED),
        ("usb-06", USB_CURRENT_NAME),
    ] {
        let all = frames(json);
        for (dir, bytes) in &all {
            let f = Frame::decode(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(hid::output_report(bytes).is_ok(), "{name}: frame too long");
            let expected = if dir == "tx" { (0, 1) } else { (1, 0) };
            assert_eq!((f.src, f.dst), expected, "{name}");
        }
        // One reply per request, except the unanswered 0x11 at the end.
        let tx = all.iter().filter(|(d, _)| d == "tx").count();
        let rx = all.iter().filter(|(d, _)| d == "rx").count();
        let silent = all
            .iter()
            .filter(|(d, b)| d == "tx" && b[5] == 0x11)
            .count();
        assert_eq!(tx, rx + silent, "{name}");
    }
}

#[test]
fn usb_full_read_is_the_factory_default_program() {
    // The first capture with all 19 pages (the network ones lost some).
    let asm = program_from(USB_FULL_READ);
    assert!(asm.missing_pages().is_empty());
    assert_eq!(asm.program().unwrap(), Program::factory_default());
}

#[test]
fn mistaken_store_left_u01_loaded() {
    // usb-01 read preset 0. After `0x26` + `0x20 01` (meant as a store) the
    // unit runs U01, whose name did not change: `0x20` loads (protocol.md §4.2).
    let replies: Vec<Reply> = frames(USB_AFTER_STORE)
        .iter()
        .filter(|(d, _)| d == "rx")
        .map(|(_, b)| Reply::parse(&Frame::decode(b).unwrap()))
        .collect();
    assert!(replies.contains(&Reply::CurrentPreset(1)));
    assert!(replies.contains(&Reply::PresetName {
        index: 1,
        name: "Default Preset".into()
    }));
    let before: Vec<Reply> = frames(USB_FULL_READ)
        .iter()
        .filter(|(d, _)| d == "rx")
        .map(|(_, b)| Reply::parse(&Frame::decode(b).unwrap()))
        .collect();
    assert!(before.contains(&Reply::CurrentPreset(0)));
    for (dir, bytes) in frames(USB_AFTER_STORE) {
        if dir == "tx" {
            assert!(READ_ONLY_CODES.contains(&bytes[5]), "{bytes:02x?}");
        }
    }
}

fn replies(json: &str) -> Vec<Reply> {
    frames(json)
        .iter()
        .filter(|(d, _)| d == "rx")
        .map(|(_, b)| Reply::parse(&Frame::decode(b).unwrap()))
        .collect()
}

#[test]
fn preset_names_and_slots_count_u01_from_zero() {
    // usb-05: the `0x22` table flags exactly the presets that carry a user
    // name, both counted from 0 = U01 (protocol.md §4.2).
    let stored = replies(USB_STORED);
    let slots = stored
        .iter()
        .find_map(|r| match r {
            Reply::PresetSlots(v) => Some(v.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(slots.len(), 20);
    let mut names = std::collections::BTreeMap::new();
    for r in &stored {
        if let Reply::PresetName { index, name } = r {
            names.insert(*index, name.clone());
        }
    }
    assert_eq!(names.len(), 21);
    for (i, flag) in slots.iter().enumerate() {
        let user = names[&(i as u8 + 1)] != "Default Preset";
        assert_eq!(*flag == 0xff, user, "U{:02}", i + 1);
    }
    assert_eq!(names[&1], "Full Stacks");
    assert_eq!(names[&2], "Test Muted");
    // Index 21 (wire 20) is not a preset but the current program's name,
    // bytes 2..16 of page 0 (memory-map.md).
    let after = replies(USB_CURRENT_NAME);
    let program_name = after
        .iter()
        .find_map(|r| match r {
            Reply::ProgramPage { page: 0, data } => Some(name_text(&data[2..16])),
            _ => None,
        })
        .unwrap();
    assert_eq!(program_name, "Muted");
    assert!(after.contains(&Reply::PresetName {
        index: 21,
        name: program_name.clone()
    }));
    assert_eq!(names[&21], program_name);
}

#[test]
fn end_of_transfer_is_acknowledged() {
    // 0x12 got `10 02 01 00 01 01 10 03 00` over USB (protocol.md §4.1).
    let all = frames(USB_FULL_READ);
    let i = all
        .iter()
        .position(|(d, b)| d == "tx" && b[5] == 0x12)
        .unwrap();
    assert_eq!(
        all[i + 1].1,
        [0x10, 0x02, 0x01, 0x00, 0x01, 0x01, 0x10, 0x03, 0x00]
    );
}

#[test]
fn usb_reports_carry_one_frame_then_stale_padding() {
    // Raw 64-byte input reports from the unit (protocol.md §1.2).
    let mut stale = 0;
    for line in USB_MUTE.lines().filter(|l| l.contains("\"report\"")) {
        let report = hex(field(line, "report").unwrap());
        let frame = hex(field(line, "hex").unwrap());
        assert_eq!(report.len(), hid::REPORT_LEN);
        assert_eq!(hid::input_frame(&report), &frame[..]);
        let padding = &report[frame.len()..];
        if padding.windows(2).any(|w| w == [0x10, 0x03]) {
            stale += 1; // leftovers of earlier, longer frames
        }
        // The last 4 bytes are past every frame the unit sends (60 bytes max).
        assert_eq!(&report[60..], [0x00, 0x49, 0xb4, 0x53]);
    }
    assert_eq!(
        stale, 3,
        "the short replies (connect, two ACKs) and none else"
    );
}

#[test]
fn usb_mute_out6_reads_back_in_the_output_mute_mask() {
    // memory-map.md §3: 0x382 = output mute mask, bit 5 = Out6.
    let pages: Vec<Vec<u8>> = frames(USB_MUTE)
        .into_iter()
        .filter_map(
            |(dir, bytes)| match Reply::parse(&Frame::decode(&bytes).unwrap()) {
                Reply::ProgramPage { page: 17, data } if dir == "rx" => Some(data),
                _ => None,
            },
        )
        .collect();
    let mask = |data: &Vec<u8>| u16::from_le_bytes([data[0x382 - 850], data[0x382 - 850 + 1]]);
    let masks: Vec<u16> = pages.iter().map(mask).collect();
    assert_eq!(masks, [0x0000, 0x0020, 0x0000]);
}

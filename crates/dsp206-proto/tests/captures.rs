//! Replays frames captured on a real DSP 206 (tools/re/captures/*.json).
//! These are the reference vectors: if a change breaks them, the change is wrong
//! or the capture must be re-checked on hardware.

#![allow(clippy::expect_used, clippy::unwrap_used)] // tests may panic

use dsp206_proto::command::{Reply, READ_ONLY_CODES};
use dsp206_proto::program::{name_text, Program, ProgramAssembler};
use dsp206_proto::units::{eq_gain_db, gain_db, q_value, threshold_db, time_ms};
use dsp206_proto::{Frame, FrameDecoder};

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

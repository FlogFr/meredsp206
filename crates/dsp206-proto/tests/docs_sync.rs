//! Keeps code, documentation and tools in agreement (claude/rules/testing.md).

#![allow(clippy::expect_used, clippy::unwrap_used)] // tests may panic

use dsp206_proto::command::{ALL_CODES, READ_ONLY_CODES};

const PROTOCOL_MD: &str = include_str!("../../../claude/docs/protocol.md");
const SAFETY_MD: &str = include_str!("../../../claude/rules/device-safety.md");
const PROBE_PY: &str = include_str!("../../../tools/probe/dsp206_dump.py");

fn code(c: u8) -> String {
    format!("`0x{c:02x}`")
}

#[test]
fn every_implemented_command_is_documented() {
    let doc = PROTOCOL_MD.to_lowercase();
    for c in ALL_CODES {
        assert!(
            doc.contains(&code(c)),
            "{} is implemented but missing from protocol.md",
            code(c)
        );
    }
}

#[test]
fn read_only_whitelist_matches_safety_rule_and_probe() {
    // device-safety.md lists the read-only commands in its "Allowed" section.
    let allowed = SAFETY_MD
        .split("## Ask the user first")
        .next()
        .unwrap_or_default()
        .to_lowercase();
    for c in READ_ONLY_CODES {
        assert!(
            allowed.contains(&code(c)),
            "{} missing from device-safety.md",
            code(c)
        );
    }
    // tools/probe/dsp206_dump.py has its own whitelist.
    let line = PROBE_PY
        .lines()
        .find(|l| l.starts_with("READ_ONLY = {"))
        .expect("READ_ONLY in probe");
    let mut probe: Vec<u8> = line
        .trim_start_matches("READ_ONLY = {")
        .trim_end_matches('}')
        .split(',')
        .map(|s| u8::from_str_radix(s.trim().trim_start_matches("0x"), 16).expect("hex code"))
        .collect();
    probe.sort_unstable();
    let mut ours = READ_ONLY_CODES.to_vec();
    ours.sort_unstable();
    assert_eq!(probe, ours, "probe whitelist and READ_ONLY_CODES differ");
}

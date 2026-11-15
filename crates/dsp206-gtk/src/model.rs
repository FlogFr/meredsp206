//! Editing state independent of GTK: the program, undo/redo, device info.

use std::time::{Duration, Instant};

use dsp206_proto::program::{name_text, preset_name_field, PAGE_SIZE, PROGRAM_SIZE};
use dsp206_proto::{Program, Request};

/// Edits to the same parameter within this window form one undo step
/// (dragging a fader produces one step, not hundreds).
const MERGE_WINDOW: Duration = Duration::from_millis(1200);
const UNDO_LIMIT: usize = 200;

pub struct Model {
    pub program: Program,
    undo: Vec<Program>,
    redo: Vec<Program>,
    last_edit: Option<((u8, u8, u8), Instant)>,
    pub preset_names: Vec<String>,
    pub current_preset: Option<u8>,
    pub identity: Option<String>,
    /// A store waiting for its preset name to be read back: slot and name.
    pending_store: Option<(u8, String)>,
}

impl Default for Model {
    fn default() -> Self {
        // The unit does not report F00's name: it is the factory program.
        let mut preset_names = vec![String::new(); 21];
        preset_names[0] = Program::factory_default().name_text();
        Model {
            program: Program::factory_default(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            preset_names,
            current_preset: None,
            identity: None,
            pending_store: None,
        }
    }
}

impl Model {
    /// Apply a setter. Returns it when the program changed (to send it).
    pub fn edit(&mut self, req: Request) -> Option<Request> {
        self.edit_at(req, Instant::now())
    }

    pub fn edit_at(&mut self, req: Request, now: Instant) -> Option<Request> {
        let before = self.program.clone();
        if !self.program.apply(&req) {
            return None;
        }
        let key = req.coalesce_key();
        let merge = matches!((key, self.last_edit), (Some(k), Some((lk, t))) if k == lk && now.duration_since(t) < MERGE_WINDOW);
        if !merge {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.last_edit = key.map(|k| (k, now));
        self.redo.clear();
        Some(req)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Go back one step; returns the setters that bring the device along.
    pub fn undo(&mut self) -> Vec<Request> {
        let Some(prev) = self.undo.pop() else {
            return Vec::new();
        };
        let reqs = self.program.diff_requests(&prev);
        self.redo.push(std::mem::replace(&mut self.program, prev));
        self.last_edit = None;
        reqs
    }

    pub fn redo(&mut self) -> Vec<Request> {
        let Some(next) = self.redo.pop() else {
            return Vec::new();
        };
        let reqs = self.program.diff_requests(&next);
        self.undo.push(std::mem::replace(&mut self.program, next));
        self.last_edit = None;
        reqs
    }

    /// Replace the program with what the device holds (not undoable).
    pub fn set_from_device(&mut self, p: Program) {
        self.program = p;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
    }

    /// Overlay one received page on the current program.
    pub fn apply_page(&mut self, page: u8, data: &[u8]) {
        let mut bytes = self.program.to_bytes();
        bytes.resize(PAGE_SIZE * dsp206_proto::program::PAGE_COUNT, 0);
        let start = page as usize * PAGE_SIZE;
        if let Some(slot) = bytes.get_mut(start..start + data.len()) {
            slot.copy_from_slice(data);
        }
        if let Ok(p) = Program::from_bytes(&bytes[..PROGRAM_SIZE]) {
            self.program = p;
        }
    }

    /// Load a program from a file (undoable); returns the setters to send.
    pub fn load(&mut self, p: Program) -> Vec<Request> {
        let reqs = self.program.diff_requests(&p);
        self.undo.push(std::mem::replace(&mut self.program, p));
        self.redo.clear();
        self.last_edit = None;
        reqs
    }

    /// Store the current program into user preset `n` (1 = U01). The program
    /// takes the new name, as in the original editor. Returns the requests:
    /// the store itself, then reads that let [`Model::store_outcome`] check it.
    pub fn begin_store(&mut self, n: u8, name: &str) -> Vec<Request> {
        self.program.name = preset_name_field(name);
        let name = name_text(&self.program.name);
        let mut reqs = dsp206_link::store_preset_sequence(n, &name);
        reqs.extend([Request::GetPresetName(n), Request::GetCurrentPreset]);
        self.pending_store = Some((n, name));
        reqs
    }

    /// Message for a preset name read back from the device, when it ends a
    /// pending store.
    pub fn store_outcome(&mut self, index: u8, name: &str) -> Option<String> {
        let (n, expected) = self.pending_store.take_if(|(n, _)| *n == index)?;
        let label = Self::preset_label(n);
        Some(if name == expected {
            format!("Stored {label} “{name}”")
        } else {
            format!("{label} was not stored: the unit still names it “{name}”")
        })
    }

    /// Message for a preset operation the device did not acknowledge.
    pub fn unconfirmed(&mut self, req: &Request) -> Option<String> {
        match req {
            Request::StorePreset(_) => {
                let (n, _) = self.pending_store.take()?;
                Some(format!(
                    "The unit did not confirm storing {}",
                    Self::preset_label(n)
                ))
            }
            Request::RecallPreset(n) => Some(format!(
                "The unit did not confirm loading {}",
                Self::preset_label(*n)
            )),
            _ => None,
        }
    }

    pub fn preset_label(index: u8) -> String {
        if index == 0 {
            "F00".into()
        } else {
            format!("U{index:02}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsp206_proto::Channel;

    fn gain(ch: u8, raw: u16) -> Request {
        Request::SetGain {
            ch: Channel::new(ch).unwrap(),
            raw,
        }
    }

    #[test]
    fn fader_drag_is_one_undo_step() {
        let mut m = Model::default();
        let t0 = Instant::now();
        for (i, raw) in (200..260).enumerate() {
            assert!(m
                .edit_at(gain(2, raw), t0 + Duration::from_millis(i as u64 * 20))
                .is_some());
        }
        assert_eq!(m.program.outputs[0].gain, 259);
        let reqs = m.undo();
        assert_eq!(reqs, vec![gain(2, 280)]);
        assert_eq!(m.program, Program::factory_default());
        assert!(!m.can_undo());
        assert_eq!(m.redo(), vec![gain(2, 259)]);
    }

    #[test]
    fn separate_parameters_are_separate_steps() {
        let mut m = Model::default();
        let t0 = Instant::now();
        m.edit_at(gain(2, 100), t0);
        m.edit_at(gain(3, 100), t0);
        m.edit_at(gain(2, 150), t0 + Duration::from_secs(5));
        assert_eq!(m.undo(), vec![gain(2, 100)]);
        assert_eq!(m.undo(), vec![gain(3, 280)]);
        assert_eq!(m.undo(), vec![gain(2, 280)]);
        assert!(m.undo().is_empty());
    }

    #[test]
    fn unchanged_edit_is_not_sent() {
        let mut m = Model::default();
        assert_eq!(m.edit(gain(0, 280)), None);
        assert!(!m.can_undo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut m = Model::default();
        m.edit(gain(0, 10));
        m.undo();
        assert!(m.can_redo());
        m.edit(gain(1, 10));
        assert!(!m.can_redo());
    }

    #[test]
    fn pages_overlay_and_load() {
        let mut m = Model::default();
        let mut target = Program::factory_default();
        target.inputs[0].gain = 333;
        let pages = target.to_pages();
        let page = 0x94 / PAGE_SIZE; // InA gain lives in page 2
        m.apply_page(page as u8, &pages[page]);
        assert_eq!(m.program.inputs[0].gain, 333);
        let mut other = Program::factory_default();
        other.outputs[5].gain = 1;
        let reqs = m.load(other.clone());
        assert!(reqs.contains(&gain(7, 1)) && reqs.contains(&gain(0, 280)));
        assert_eq!(m.program, other);
        m.undo();
        assert_eq!(m.program.inputs[0].gain, 333);
    }

    #[test]
    fn store_names_the_program_and_checks_the_read_back() {
        let mut m = Model::default();
        let reqs = m.begin_store(1, "Club ");
        assert_eq!(
            reqs,
            [
                Request::SetPresetName(*b"Club          "),
                Request::StorePreset(1),
                Request::EndOfTransfer,
                Request::GetPresetName(1),
                Request::GetCurrentPreset,
            ]
        );
        assert_eq!(m.program.name_text(), "Club");
        // Another slot's name does not end the store.
        assert_eq!(m.store_outcome(2, "Club"), None);
        assert_eq!(
            m.store_outcome(1, "Club").as_deref(),
            Some("Stored U01 “Club”")
        );
        assert_eq!(m.store_outcome(1, "Club"), None);
        m.begin_store(3, "Live");
        assert_eq!(
            m.store_outcome(3, "Default Preset").as_deref(),
            Some("U03 was not stored: the unit still names it “Default Preset”")
        );
    }

    #[test]
    fn unconfirmed_preset_operations_are_reported() {
        let mut m = Model::default();
        m.begin_store(2, "A");
        assert_eq!(
            m.unconfirmed(&Request::StorePreset(2)).as_deref(),
            Some("The unit did not confirm storing U02")
        );
        assert_eq!(m.store_outcome(2, "A"), None);
        assert_eq!(m.unconfirmed(&Request::StorePreset(2)), None);
        assert_eq!(
            m.unconfirmed(&Request::RecallPreset(0)).as_deref(),
            Some("The unit did not confirm loading F00")
        );
        assert_eq!(m.unconfirmed(&gain(0, 1)), None);
    }

    #[test]
    fn preset_labels() {
        assert_eq!(Model::default().preset_names[0], "Default Preset");
        assert_eq!(Model::preset_label(0), "F00");
        assert_eq!(Model::preset_label(7), "U07");
    }
}

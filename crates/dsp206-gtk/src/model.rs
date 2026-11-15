//! Editing state independent of GTK: the program, undo/redo, device info.

use std::time::{Duration, Instant};

use dsp206_proto::program::{PAGE_SIZE, PROGRAM_SIZE};
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
}

impl Default for Model {
    fn default() -> Self {
        Model {
            program: Program::factory_default(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            preset_names: vec![String::new(); 21],
            current_preset: None,
            identity: None,
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
    fn preset_labels() {
        assert_eq!(Model::preset_label(0), "F00");
        assert_eq!(Model::preset_label(7), "U07");
    }
}

//! Pure, I/O-free implementation of the t.racks DSP 206 control protocol.
//!
//! Every constant and layout here comes from `claude/docs/protocol.md` and
//! `claude/docs/memory-map.md`. Keep them in sync: a fact used in code must be
//! documented there, and tests must cover it.

pub mod command;
pub mod edit;
pub mod frame;
pub mod hid;
pub mod program;
pub mod units;

pub use command::{Channel, Meters, Reply, Request, ToneSource};
pub use frame::{Frame, FrameDecoder, FrameError};
pub use hid::HidError;
pub use program::{Program, ProgramAssembler, ProgramError};

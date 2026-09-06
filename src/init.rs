//! Turning a config file somebody already wrote into a template, a theme and a target.
//!
//! `docs/init.md` decides the dialogue, which notations are substituted, where the three
//! files go, and that nothing is written until the template renders back to the original
//! byte for byte.

mod answer;
mod emit;
mod naming;
mod plan;
mod scan;
mod weave;

pub use answer::{Answer, AnswerError, answer};
pub use emit::EmitError;
pub use naming::{display_name, suggest};
pub use plan::{Draft, InitError, Plan, Written, plan};
pub use scan::{Colour, Occurrence, Scan, Unhandled, scan};
pub use weave::{Binding, weave};

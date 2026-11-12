//! craft-bridge: pack SolidCraft designs into craftx artifacts and inspect
//! them. Replay, not translation — see `docs/spec.md` and
//! `docs/replay-profile.md`.

pub mod build;
pub mod error;
pub mod inspect;
pub mod limits;
pub mod pack;
pub mod profile;
pub mod recipe;
pub mod replay;
pub mod report;
pub mod swb;
pub mod zipio;

pub use error::{Error, Result};

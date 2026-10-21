//! Contracts shared by every Nudgy module.
//!
//! These Rust types are the single source of truth. `cargo test -p nudgy-shared`
//! regenerates the TypeScript bindings in `ts/generated/` and type-checks the
//! wire fixtures in `fixtures/` on both sides. Changing anything here changes a
//! contract: every module that consumes it must be updated in the same change.
//!
//! Wire conventions: struct fields are camelCase, enum values are snake_case,
//! timestamps are Unix milliseconds (UTC), durations are milliseconds, and
//! absent optional fields are omitted rather than sent as `null`.

pub mod activity;
pub mod classify;
pub mod common;
pub mod constants;
pub mod line;
pub mod overlay;
pub mod reminder;
pub mod session;
pub mod settings;
pub mod trigger;

pub use activity::*;
pub use classify::*;
pub use common::*;
pub use line::*;
pub use overlay::*;
pub use reminder::*;
pub use session::*;
pub use settings::*;
pub use trigger::*;

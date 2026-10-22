//! Milestone 3: the classifier and the session engine.
//!
//! `Classifier` labels each activity focused / neutral / distracting against
//! the goal. `Engine` is the session state machine that turns labelled
//! activity, user actions and time into triggers: when the panda speaks, and
//! about what. Both are pure; the desktop app feeds them and persists outputs.

pub mod categories;
pub mod classifier;
#[cfg(feature = "embeddings")]
pub mod embed;
pub mod engine;
pub mod present;
pub mod similarity;
pub mod streak;

pub use classifier::{Classifier, Overrides, normalize_goal};
pub use engine::{Engine, EngineSettings, Input, Output};
pub use similarity::{Lexical, Similarity};

//! Pure logic shared by the Crumb server and the native apps: recipe shapes and
//! validation, the parsers that turn pasted text into them, and Try next ranking.

pub mod categories;
pub mod checks;
pub mod client;
pub mod duration;
pub mod error;
pub mod format;
pub mod fractions;
pub mod ingredients;
pub mod markdown;
pub mod model;
pub mod source;
pub mod suggest;
pub mod sun;
pub mod text_parser;

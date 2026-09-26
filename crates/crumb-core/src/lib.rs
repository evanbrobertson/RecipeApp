//! Pure logic shared by the Crumb server and the desktop app: recipe shapes and
//! validation, the parsers that turn pasted text into them, and Try next ranking.

pub mod categories;
pub mod duration;
pub mod error;
pub mod fractions;
pub mod ingredients;
pub mod markdown;
pub mod model;
pub mod source;
pub mod suggest;
pub mod text_parser;

//! Content streams read from a `Read`, one operator at a time, with memory
//! bounded by the largest token and not by the stream.

pub mod cache;
pub mod inline;
pub mod lexer;
pub mod operators;
pub mod parts;

pub use inline::Resolver;
pub use parts::PartsReader;
pub use lexer::{Lexer, TokenKind};
pub use operators::{consumed_names, Damage, InlineImages, Mode, OperatorStream, Operators};

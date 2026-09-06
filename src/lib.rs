//! Renders a template against a theme's resolved tokens.
//!
//! The template language is `{{token}}` substitution and nothing else. See
//! `docs/theme-format.md` for the token path grammar and the escape rules.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod template;
pub mod token;

pub use template::{Template, TemplateError, UndefinedToken};
pub use token::{TokenPath, Tokens};

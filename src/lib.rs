//! Reads a theme, and renders a template against the tokens it resolves to.
//!
//! The template language is `{{token}}` substitution and nothing else. See
//! `docs/theme-format.md` for the theme file, the token path grammar and the escape rules.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod catalog;
pub mod paths;
pub mod state;
pub mod template;
pub mod theme;
pub mod token;

pub use catalog::{Catalog, CatalogError};
pub use paths::{Environment, PathsError};
pub use state::{State, StateError};
pub use template::{Template, TemplateError, UndefinedToken};
pub use theme::{Problem, Theme, ThemeError, ThemeId, Variant};
pub use token::{TokenPath, Tokens};

//! Reads a theme, and renders a template against the tokens it resolves to.
//!
//! The template language is `{{token}}` substitution and nothing else. See
//! `docs/theme-format.md` for the theme file, the token path grammar and the escape rules.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod apply;
pub mod catalog;
pub mod check;
pub mod config;
pub mod diff;
pub mod import;
pub mod init;
pub mod paths;
pub mod remote;
pub mod scheme;
pub mod state;
pub mod template;
pub mod theme;
pub mod token;
pub mod vocabulary;

pub use apply::{Applied, ApplyError, Plan, Rendered, apply, plan, render};
pub use catalog::{Catalog, CatalogError};
pub use check::{CheckError, Disk, Finding, Report, check, compare};
pub use config::{Config, ConfigError, Target, TargetName};
pub use diff::unified;
pub use import::{ImportError, Imported, import};
pub use init::{Answer, Binding, Colour, InitError, Occurrence, Scan, Unhandled};
pub use paths::{Environment, PathsError};
pub use remote::{Installed, RemoteError};
pub use scheme::cache::{Cache, CacheError};
pub use scheme::{ConvertError, Converted, Family, Scheme, SchemeError, System, convert};
pub use state::{State, StateError};
pub use template::{Template, TemplateError, UndefinedToken};
pub use theme::{Problem, Theme, ThemeError, ThemeId, Variant};
pub use token::{TokenPath, Tokens};

---
paths:
  - "**/*.rs"
---

# Error handling

## Library layer vs application layer

- Domain error types are defined with `thiserror`, one per module or domain boundary.
- `anyhow` appears only at the application boundary — `main.rs` and top-level orchestration.
- Everything below that returns typed errors. `anyhow::Result` is not allowed there.

## Designing error types

- One error enum per module or domain boundary, e.g. `ThemeError`, `TemplateError`, `ConfigError`.
- Every variant carries enough context to act on.
- Use `#[from]` for automatic conversion from source errors.
- Reserve `#[error(transparent)]` for opaque catch-all variants.

## Forbidden

- `panic!()` in library code — return a `Result`.
- String-based errors in the library layer, e.g. `anyhow!("something failed")`.
- Swallowing a meaningful error with `.unwrap_or_default()`.

## Adding context

- Use `.context()` / `.with_context()` when propagating upward.
- A message must say *which operation failed* and *on what input*.
- Prefer a structured variant over string interpolation when the error will be handled programmatically.

## Messages

- Report **every** problem found in a pass, not the first one. One undefined token must not hide the other four.
- Name the offending token, the template path, and the line.
- Never fail halfway through writing. If any target fails to render, write nothing.

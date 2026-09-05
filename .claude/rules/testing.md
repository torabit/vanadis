---
paths:
  - "**/*.rs"
  - "tests/**"
---

# Testing

## Structure

- Unit tests in `#[cfg(test)] mod tests` inside each module.
- Integration tests in `tests/`, driven by `tests/fixtures/`.
- `cargo test` must pass on a fresh clone with no network access.
- Reach for `trybuild` if a macro or a complex type-level API is ever introduced.

## Quality

- Test behaviour, not implementation details.
- One assertion per test where that is practical.
- Test the error paths, not only the happy path.
- Property-based tests (`proptest` / `quickcheck`) for pure logic — reference resolution and
  cycle detection are the obvious candidates.

## Golden data

`tests/fixtures/expected/` holds what the reference JavaScript renderer produces from
`tests/fixtures/templates/` and `tests/fixtures/palette.json`.

**Do not regenerate those files to make a test pass.** A diff means the renderer is wrong,
not the fixture.

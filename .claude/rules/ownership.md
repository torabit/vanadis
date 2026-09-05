---
paths:
  - "**/*.rs"
---

# Ownership and lifetimes

## Avoid lifetimes on structs by default

- A struct should only carry a lifetime parameter when it lives entirely within a single
  function call and dies in that scope.
- When a struct needs external data, prefer, in order:
  - owning the data (`Clone` / `Arc`),
  - passing the reference as a function argument rather than storing it as a field.

## No parent links

- Do not store parent or upward references in tree structures.
- Pass parent context as a function argument when it is needed — the ephemeral-reference pattern.
- When bidirectional traversal is genuinely required, use indices (arena pattern) or `Arc<Mutex<_>>`.

## No mutual references between objects

- Avoid designs where two objects hold references to each other.
- Split into host plus controller and make the communication one-directional.
- Prefer channels or return values over callbacks.

## Iterators

- If something produces output that cannot be collected into a `Vec`, it is not a proper iterator.
- A streaming iterator that borrows from `self` needs GATs or a different API shape.
- Return `impl Iterator` rather than defining a custom iterator type.

## Borrowing

- Take `&str` / `&[T]` as arguments, not `&String` / `&Vec<T>`.
- Use `impl AsRef<str>` or `Into<String>` where a flexible API is worth it.
- Borrow rather than clone. Clone only at ownership boundaries.

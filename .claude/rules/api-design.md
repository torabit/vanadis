---
paths:
  - "src/**/*.rs"
---

# API design

## Type safety

- Newtypes for domain concepts, e.g. `ThemeName(String)`, `Token(String)`, `TargetName(String)`.
- An enum beats a `bool` argument.
- Builder pattern once a type has three or more optional fields.
- Consider typestate for stateful workflows where compile-time safety matters.

## Trait implementations

- Implement the standard traits liberally: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default`, `Display`.
- Types should be `Send + Sync`; document the reason when that is not possible.
- Public API types implement `Serialize` / `Deserialize`.
- Keep required trait methods minimal and supply default implementations.

## Naming

- RFC 430 casing: `snake_case` for functions and variables, `CamelCase` for types.
- Conversions: `as_` (cheap reference), `to_` (expensive conversion), `into_` (takes ownership).
- Iterators: `iter()` → `&T`, `iter_mut()` → `&mut T`, `into_iter()` → `T`.
- Constructors: `new()` as the base, `with_*()` for variations, `from_*()` for conversions.

## Modules

- One public type per module as a guideline, not a rule.
- Re-export the important types at the module root.
- Consider splitting a file once it passes roughly 300 lines.
- `pub(crate)` for internal items, not `pub`.
- Explicit imports over wildcards.

## Documentation

- Doc comments on every public item.
- Examples in `///` use `?`, never `unwrap()`.
- Document panic conditions, errors, and safety invariants.

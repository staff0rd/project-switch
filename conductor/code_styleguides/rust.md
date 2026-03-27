# Rust Style Guide

This document summarizes key rules and best practices for writing idiomatic Rust code in this project.

## 1. Formatting

- **`rustfmt`:** All Rust code **must** be formatted with `cargo fmt`. Enforced by `verify:fmt`.
- **Indentation:** 4 spaces per indentation level. No tabs.
- **Line Length:** Maximum 100 characters (rustfmt default).
- **Trailing Commas:** Use trailing commas in multi-line constructs (structs, enums, function args).

## 2. Linting

- All code must pass `cargo clippy -- -D warnings`. Enforced by `verify:clippy`.
- Do not blanket-allow clippy lints. If a specific lint must be suppressed, use `#[allow(clippy::lint_name)]` with a comment explaining why.

## 3. Naming

- **`snake_case`:** Functions, methods, variables, modules, and crate names.
- **`PascalCase` (UpperCamelCase):** Types, traits, enum variants, and type parameters.
- **`SCREAMING_SNAKE_CASE`:** Constants and statics.
- **Conversions:** Use `as_`, `to_`, `into_` prefixes following Rust conventions:
  - `as_` — cheap, borrowed view (e.g., `as_str()`)
  - `to_` — expensive conversion, or borrowed-to-owned (e.g., `to_string()`)
  - `into_` — owned conversion consuming self (e.g., `into_inner()`)
- **Getters:** Do not use a `get_` prefix. Name the getter after the field (e.g., `fn name(&self) -> &str`).

## 4. Error Handling

- Use `Result<T, E>` for recoverable errors. Use `anyhow::Result` for application-level error propagation.
- Use the `?` operator for error propagation. Avoid `.unwrap()` and `.expect()` in production code — reserve them for cases where failure is genuinely impossible.
- Provide context with `anyhow::Context` (`.context("message")` / `.with_context(|| format!(...))`) when propagating errors.
- Do not use `panic!` for expected error conditions.

## 5. Ownership & Borrowing

- Prefer borrowing (`&T`, `&mut T`) over cloning when possible.
- Use `Clone` explicitly — do not hide copies behind method calls.
- Prefer `&str` over `String` in function parameters when the function does not need ownership.

## 6. Types & Structs

- Derive common traits in a consistent order: `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize`.
- Use tuple structs for simple wrappers (newtypes).
- Prefer enums over boolean flags when there are more than two states.
- Use `Option<T>` instead of sentinel values.

## 7. Functions & Methods

- Keep functions small and focused on a single responsibility.
- Prefer returning `impl Iterator` over collecting into a `Vec` when the caller may not need all items.
- Use iterators and combinators (`.map()`, `.filter()`, `.collect()`) over manual loops when it improves clarity.
- Avoid more than 3-4 parameters — use a builder or config struct instead.

## 8. Modules & Visibility

- One module per file. Use `mod.rs` or the filename convention consistently within the project.
- Keep `pub` visibility as narrow as possible. Default to private.
- Re-export important types at the crate root or module level for a clean public API.

## 9. Dependencies

- Prefer well-maintained, widely-used crates.
- Enable only the features you need (e.g., `clap = { features = ["derive"] }`).
- No unused dependencies. Enforced by `verify:machete`.
- No duplicate code across `.rs` files. Enforced by `verify:duplicate-code`.

## 10. Unsafe

- Avoid `unsafe` unless absolutely necessary.
- If `unsafe` is required, document the safety invariants in a `// SAFETY:` comment directly above the block.
- Encapsulate `unsafe` behind safe abstractions.

## 11. Testing

- Place unit tests in a `#[cfg(test)] mod tests` block at the bottom of each file.
- Use `#[test]` functions with descriptive names (e.g., `fn parses_empty_config_as_default()`).
- Test both success and failure paths.
- Use `assert_eq!` and `assert!` with meaningful failure messages where helpful.
- All tests must pass. Enforced by `test`.

## 12. Security

- All dependencies are audited for known vulnerabilities. Enforced by `verify:audit`.

**BE CONSISTENT.** When editing code, match the existing style.

_Sources: [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/), [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)_

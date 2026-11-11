# Contributing to WikiForge

Thank you for helping. Please read `AGENTS.md` for the architecture, invariants and conventions
(it applies to human contributors as well), and follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Getting started

Install Rust 1.95 or newer, then:

```
git clone https://github.com/HyrumHendrickson/WikiForge
cd WikiForge
cargo run --release
```

On Linux you need the windowing libraries listed in the README.

## Before you open a pull request

```
cargo fmt --all
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node --check assets/index.js && node --check assets/page/index.js
```

CI runs the same checks. Keep pull requests focused: one logical change, no unrelated reformatting.
Add or update tests, update `CHANGELOG.md` under *Unreleased*, and update the README when behavior
changes.

## Adding a block or embed type

1. Add a `Kind` variant and its parse, serialize and default text in `src/model.rs`; add a round-trip test.
2. Add it to `ADD_KINDS` and render its editor widget in `src/app.rs`.
3. Render it in `assets/page/index.js` and initialize any widget in `initWidgets`.
4. Add an example to `assets/showcase.md` and document the syntax in the README table.
5. If it loads a remote script, add the host to `CONFIG`, pin the version, and note it in `SECURITY.md`.

## Reporting bugs and proposing features

Use the issue templates. For bugs include your OS, the WikiForge commit or version, and the steps
to reproduce. Open an issue to discuss large features before writing code.

## Licensing

By contributing you agree that your contribution is licensed under the MIT License (see `LICENSE`).

# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims
to follow [Semantic Versioning](https://semver.org/) once it reaches 1.0.

## [Unreleased]

### Added
- Custom site logo: **Site > Set logo...** and **Reset logo to default**. The logo appears in the
  header and as the browser tab icon of the home page and every article.
- WikiForge logo as the default site logo and as the editor window icon.
- Project documentation: `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`.
- Continuous integration (format, clippy, tests, MSRV, JS syntax), dependency auditing, CodeQL
  and Dependabot configuration, issue and pull request templates.
- `rustfmt.toml`, `.editorconfig`, `.gitattributes` and crate metadata in `Cargo.toml`.

### Changed
- The left sidebar lists featured articles only; the Navigation block is removed.
- New sites start with no articles and no featured articles.
- `page/` without `?p=` redirects to the home page.

### Removed
- The starter "Main Page" article. The home page with the site map is the only main page.

### Fixed
- Clippy warnings in the editor UI.

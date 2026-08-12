# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Composite GitHub Action at the repository root that installs `envie` from
  GitHub Releases (checksum-verified) and can run a command (`command`, `env`,
  `unit`, `dry-run`, `no-prompt`, `override`). `args` remains an escape hatch.
- MIT `LICENSE` file matching the crate metadata.
- This changelog.

## [0.2.1] - 2026-08-11

### Added

- `envie output --format` for table, json, yaml, and env.
- Logo on the README and example guides.

### Changed

- `envie show` describes the project from any directory inside it, not only
  the root.
- Guides are checked in CI against the commands and files the tool actually
  produces.

## [0.2.0] - 2026-08-11

### Added

- `envie list` reports declared and deployed environments, including
  ephemeral ones that exist only because somebody deployed them.

### Changed

- Homebrew formula generated so `brew audit --strict` accepts it.

## [0.1.0] - 2026-08-11

First release. Envie runs many environments from one Terraform codebase,
including repositories it did not create: adoption reads the backend,
workspace, state keys and variable values a repository already uses, so
deploying the environment you adopted changes nothing, while new environments
get their own state and their own names.

[Unreleased]: https://github.com/fearlessfara/envie/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/fearlessfara/envie/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/fearlessfara/envie/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fearlessfara/envie/releases/tag/v0.1.0

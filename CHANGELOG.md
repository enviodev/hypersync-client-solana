# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Cargo workspace at the repo root covering all three crates.
- MPL-2.0 LICENSE, README, CHANGELOG.
- Per-crate metadata (description, license, repository, homepage, keywords,
  categories, readme).

### Fixed

- Client `Cargo.toml` path dependencies now resolve inside this repo when used
  standalone (previously only resolved as a path dep from the main hypersync repo).

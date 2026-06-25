# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Entry labels**

- `Release`: Package/version metadata and publishing preparation.
- `Library`: Runtime behavior, public API, protocol handling, or validation in the distributed library.
- `Docs`: README, user guides, generated API docs, or other documentation-only changes.
- `Samples`: Examples, sample flows, sample scripts, or sample applications.
- `Tests`: Test suites, test fixtures, golden vectors, or verification data.
- `Tooling`: Developer/operator command-line tools and helper utilities.
- `CI`: Release checks, workflow scripts, or automation-only changes.

## [Unreleased] - 2026-06-25

### Changed
- Docs: Refreshed Host Link getting-started, gotchas, supported-register, and usage guidance.
- Samples: Updated Host Link examples to use safer write/restore patterns.

## [1.0.0] - 2026-06-24

### Changed
- Release: Bumped crate metadata and lockfile metadata to `1.0.0` for the first stable release line.

### Fixed
- Tests: Aligned Host Link frame-vector entries that share IDs with the .NET/Python vector sets, including `wre_dm100_vals` and `check_error_no`.

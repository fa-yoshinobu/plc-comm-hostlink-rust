# Release Process

This checklist governs crates.io and GitHub publication for this repository.

## Pre-Tag Gate

1. Use a clean release branch and run `release_check.bat` before the version exists in crates.io.
2. Confirm `Cargo.toml`, runtime version metadata, CHANGELOG, user docs, examples, and public API agree.
3. Enumerate every unchecked repository TODO and maintainer checkbox. Pass it, mark it explicitly not required, or record an item-by-item release disposition in the active release GOAL.
4. Run `cargo package` and inspect the crate before creating the immutable annotated tag.

## Publication Integrity Gate

1. Publish only after inspecting the GitHub `.crate` asset.
2. Download the crates.io package and compare its extracted file set and normalized text contents with the GitHub asset. A compression or line-ending difference is not evidence of a code difference.
3. Build the shared docs site in a fresh virtual environment and require the Python package version/symbol check plus `mkdocs build --strict`.
4. Verify the fixed tag target, final Release state/assets, docs deployment, open release PR count, and clean working tree.
5. List every permitted unverified hardware scope in the final release summary; never convert it to a live pass.

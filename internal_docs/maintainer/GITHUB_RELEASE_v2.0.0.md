# v2.0.0

## BREAKING

crates.io package renamed and Rust import path changed.

| Old crate/use | New crate/use |
| --- | --- |
| `plc-comm-hostlink-rust` | `plc-comm-kv-hostlink` |
| `use plc_comm_hostlink::...` | `use plc_comm_kv_hostlink::...` |

## Highlights

- Version metadata bumped to 2.0.0.
- Release duplicate checks now target `plc-comm-kv-hostlink`.
- README, Getting Started, examples, tests, and docs.rs links use the new crate/import names.

Package matrix: https://fa-yoshinobu.github.io/plc-comm-docs-site/package-matrix/

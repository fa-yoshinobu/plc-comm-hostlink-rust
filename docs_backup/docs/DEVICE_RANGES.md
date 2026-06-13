# Device Range Catalog

This document describes the static device-range catalog used by:

- `device_range_catalog_for_plc_profile(plc_profile)`

The current catalog is embedded directly in `src/device_ranges.rs`. The library
returns `slmp-rust`-style range metadata so the same UI code can consume both
protocols with minimal branching:

- `KvDeviceRangeCatalog`
- `KvDeviceRangeCategory`
- `KvDeviceRangeEntry`
- `KvDeviceRangeSegment`
- `KvDeviceRangeNotation`

## Behavior

- `KvDeviceRangeCatalog.plc_profile` is the resolved canonical profile such as `keyence:kv-8000`.
- `KvDeviceRangeCatalog.model_code` is empty for static profile catalogs.
- `KvDeviceRangeEntry.device` follows the published alias when the row maps to a single alias device.
  Examples: `DM(XYM)` -> `D`, `FM(XYM)` -> `F`.
- `KvDeviceRangeEntry.device_type` preserves the original catalog row such as `DM` or `R`.
- `supported = false` means the source cell was `-`.
- `address_range` preserves the published catalog text.
- `lower_bound`, `upper_bound`, and `point_count` are parsed from that published range text.
- `segments` splits comma-separated alias ranges such as `X0-999F,Y0-999F`.
- `entry(device)` matches the original row, the primary published device, or any segment alias.
- `notation` follows the published device notation. Most rows match the `Base` column directly.
  XYM alias devices such as `X` and `Y` expose a hexadecimal low bit digit, but the digits before
  that bit are decimal (`X1999F` means bank `1999`, bit `F`).
- For XYM `X`/`Y` ranges, `lower_bound`, `upper_bound`, and `point_count` use the linear parsed
  index (`bank * 16 + bit`) while `address_range` keeps the published text.
- Multi-alias rows such as `R(XYM)` keep `device = R` and publish alias details in `segments` with a note.

## Source Corrections

The original source sheet contained a few obvious typos. The embedded catalog
and the tables below already reflect the corrected values.

| Location | Original value | Corrected value | Reason |
| --- | --- | --- | --- |
| `CR` row, `KV-3000/5000(XYM)` | `CR0000-153915` | `CR0000-CR3915` | The missing `CR` prefix is inconsistent with the same row and family. |
| `CM` row, `KV-NANO` | `CR0000-CR8999` | `CM0000-CM8999` | The row is `CM`, so the `CR` device prefix was treated as a typo. |
| `CM` row, `KV-NANO(XYM)` | `CR0000-CR8999` | `CM0000-CM8999` | Same typo as the standard `KV-NANO` column. |
| `FM` row, `KV-3000/5000(XYM)` | `E0-32767` | `F0-32767` | The row is `FM`, so the `E` alias was treated as a typo and corrected to `F`. |
| `VM` row | `0-...` | `VM0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `VB` row | `0-...` | `VB0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `CTH` row | `0-...` | `CTH0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `CTC` row | `0-...` | `CTC0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `AT` row | `0-...` | `AT0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |

## PLC Profiles

The public API accepts canonical `PlcProfile` strings only. Legacy KEYENCE model
labels such as `KV-X500` are not accepted as public profile input.

Supported profiles:

- `keyence:kv-nano`
- `keyence:kv-nano-xym`
- `keyence:kv-3000-5000`
- `keyence:kv-3000-5000-xym`
- `keyence:kv-7000`
- `keyence:kv-7000-xym`
- `keyence:kv-8000`
- `keyence:kv-8000-xym`
- `keyence:kv-x500`
- `keyence:kv-x500-xym`

Applications must choose one of the canonical profiles explicitly. The runtime
`?K` command remains available as a diagnostic, but its result is not used to
select a device-range catalog. Use this family list only when choosing which
canonical profile to pass:

- `KV-N24nn`, `KV-N40nn`, `KV-N60nn`, `KV-NC32T` -> `KV-NANO`
- `KV-3000`, `KV-5000`, `KV-5500` -> `KV-3000/5000`
- `KV-7000`, `KV-7300`, `KV-7500` -> `KV-7000`
- `KV-8000`, `KV-8000A` -> `KV-8000`
- `KV-X310`, `KV-X500`, `KV-X520`, `KV-X530`, `KV-X550` -> `KV-X500`
- Appending `(XYM)` selects the XYM catalog column for the same family.

Examples:

- `device_range_catalog_for_plc_profile("keyence:kv-x500")` selects the `KV-X500` catalog column
- `device_range_catalog_for_plc_profile("keyence:kv-3000-5000-xym")` selects the explicit XYM column

## Static Range Tables

The following Markdown tables are copied from the embedded source catalog. They
are split into standard and `XYM` catalog columns to keep them readable in
GitHub-style Markdown renderers.

### Standard Catalog Columns

| DeviceType | Base | KV-NANO | KV-3000/5000 | KV-7000 | KV-8000 | KV-X500 |
| --- | --- | --- | --- | --- | --- | --- |
| R | 10 | R00000-R59915 | R00000-R99915 | R00000-R199915 | R00000-R199915 | R00000-R199915 |
| B | 16 | B0000-B1FFF | B0000-B3FFF | B0000-B7FFF | B0000-B7FFF | B0000-B7FFF |
| MR | 10 | MR00000-MR59915 | MR00000-MR99915 | MR000000-MR399915 | MR000000-MR399915 | MR000000-MR399915 |
| LR | 10 | LR00000-LR19915 | LR00000-LR99915 | LR00000-LR99915 | LR00000-LR99915 | LR00000-LR99915 |
| CR | 10 | CR0000-CR8915 | CR0000-CR3915 | CR0000-CR7915 | CR0000-CR7915 | CR0000-CR7915 |
| CM | 10 | CM0000-CM8999 | CM0000-CM5999 | CM0000-CM5999 | CM0000-CM7599 | CM0000-CM7599 |
| T | 10 | T0000-T0511 | T0000-T3999 | T0000-T3999 | T0000-T3999 | T0000-T3999 |
| C | 10 | C0000-C0255 | C0000-C3999 | C0000-C3999 | C0000-C3999 | C0000-C3999 |
| DM | 10 | DM00000-DM32767 | DM00000-DM65534 | DM00000-DM65534 | DM00000-DM65534 | DM00000-DM65534 |
| EM | 10 | - | EM00000-EM65534 | EM00000-EM65534 | EM00000-EM65534 | EM00000-EM65534 |
| FM | 10 | - | FM00000-FM32767 | FM00000-FM32767 | FM00000-FM32767 | FM00000-FM32767 |
| ZF | 10 | - | ZF000000-ZF131071 | ZF000000-ZF524287 | ZF000000-ZF524287 | ZF000000-ZF524287 |
| W | 16 | W0000-W3FFF | W0000-W3FFF | W0000-W7FFF | W0000-W7FFF | W0000-W7FFF |
| TM | 10 | TM000-TM511 | TM000-TM511 | TM000-TM511 | TM000-TM511 | TM000-TM511 |
| VM | 10 | VM0-9499 | VM0-49999 | VM0-63999 | VM0-589823 | - |
| VB | 16 | VB0-1FFF | VB0-3FFF | VB0-F9FF | VB0-F9FF | - |
| Z | 10 | Z1-12 | Z1-12 | Z1-12 | Z1-12 | - |
| CTH | 10 | CTH0-3 | CTH0-1 | - | - | - |
| CTC | 10 | CTC0-7 | CTC0-3 | - | - | - |
| AT | 10 | - | AT0-7 | AT0-7 | AT0-7 | - |

### XYM Catalog Columns

| DeviceType | Base | KV-NANO(XYM) | KV-3000/5000(XYM) | KV-7000(XYM) | KV-8000(XYM) | KV-X500(XYM) |
| --- | --- | --- | --- | --- | --- | --- |
| R | 10 | X0-599F,Y0-599F | X0-999F,Y0-999F | X0-1999F,Y0-1999F | X0-1999F,Y0-1999F | X0-1999F,Y0-1999F |
| B | 16 | B0000-B1FFF | B0000-B3FFF | B0000-B7FFF | B0000-B7FFF | B0000-B7FFF |
| MR | 10 | M0-9599 | M0-15999 | M000000-M63999 | M000000-M63999 | M000000-M63999 |
| LR | 10 | L0-3199 | L0-15999 | L00000-L15999 | L00000-L15999 | L00000-L15999 |
| CR | 10 | CR0000-CR8915 | CR0000-CR3915 | CR0000-CR7915 | CR0000-CR7915 | CR0000-CR7915 |
| CM | 10 | CM0000-CM8999 | CM0000-CM5999 | CM0000-CM5999 | CM0000-CM7599 | CM0000-CM7599 |
| T | 10 | T0000-T0511 | T0000-T3999 | T0000-T3999 | T0000-T3999 | T0000-T3999 |
| C | 10 | C0000-C0255 | C0000-C3999 | C0000-C3999 | C0000-C3999 | C0000-C3999 |
| DM | 10 | D0-32767 | D0-65534 | D00000-D65534 | D00000-D65534 | D00000-D65534 |
| EM | 10 | - | E0-65534 | E00000-E65534 | E00000-E65534 | E00000-E65534 |
| FM | 10 | - | F0-32767 | F00000-F32767 | F00000-F32767 | F00000-F32767 |
| ZF | 10 | - | ZF000000-ZF131071 | ZF000000-ZF524287 | ZF000000-ZF524287 | ZF000000-ZF524287 |
| W | 16 | W0000-W3FFF | W0000-W3FFF | W0000-W7FFF | W0000-W7FFF | W0000-W7FFF |
| TM | 10 | TM000-TM511 | TM000-TM511 | TM000-TM511 | TM000-TM511 | TM000-TM511 |
| VM | 10 | VM0-9499 | VM0-49999 | VM0-63999 | VM0-589823 | - |
| VB | 16 | VB0-1FFF | VB0-3FFF | VB0-F9FF | VB0-F9FF | - |
| Z | 10 | Z1-12 | Z1-12 | Z1-12 | Z1-12 | - |
| CTH | 10 | CTH0-3 | CTH0-3 | - | - | - |
| CTC | 10 | CTC0-7 | CTC0-3 | - | - | - |
| AT | 10 | - | AT0-7 | AT0-7 | AT0-7 | - |

## Notes

- XYM columns may remap the same logical row to alias devices such as `X`, `Y`,
  `D`, `E`, `F`, `M`, and `L`.
- Low-level address parsing and span validation in `src/address.rs` are expected
  to stay aligned with these published XYM alias limits. In particular, `X`
  and `Y` accept `0..1999F`, `M` accepts `0..63999`, and `L` remains `0..15999`.
- The crate keeps unsupported rows in the catalog and marks them with
  `supported = false`.
- If the catalog changes, update both `src/device_ranges.rs` and this document together.

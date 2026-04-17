# Device Range Catalog

This document describes the static device-range catalog used by:

- `device_range_catalog_for_model(model)`
- `HostLinkClient::read_device_range_catalog()`
- `QueuedHostLinkClient::read_device_range_catalog()`

The current catalog is embedded directly in `src/device_ranges.rs`. The library
returns:

- `KvDeviceRangeCatalog`
- `KvDeviceRangeEntry`
- `KvDeviceRangeSegment`
- `KvDeviceRangeNotation`

## Behavior

- `supported = false` means the source cell was `-`.
- `address_range` preserves the original catalog text.
- `segments` splits comma-separated alias ranges such as `X0-999F,Y0-999F`.
- `notation` is derived from the `Base` column: `10` -> `Decimal`, `16` -> `Hexadecimal`.

## Model Resolution

The public API accepts either the exact catalog column name or a runtime model
name returned by `?K`.

Exact catalog columns:

- `KV-NANO`
- `KV-NANO(XYM)`
- `KV-3000/5000`
- `KV-3000/5000(XYM)`
- `KV-7000`
- `KV-7000(XYM)`
- `KV-8000`
- `KV-8000(XYM)`
- `KV-X500`
- `KV-X500(XYM)`

Runtime model aliases resolved by the implementation:

- `KV-N24nn`, `KV-N40nn`, `KV-N60nn`, `KV-NC32T` -> `KV-NANO`
- `KV-3000`, `KV-5000`, `KV-5500` -> `KV-3000/5000`
- `KV-7000`, `KV-7300`, `KV-7500` -> `KV-7000`
- `KV-8000`, `KV-8000A` -> `KV-8000`
- `KV-X310`, `KV-X500`, `KV-X520`, `KV-X530`, `KV-X550` -> `KV-X500`
- Appending `(XYM)` selects the XYM catalog column for the same family.

Examples:

- `device_range_catalog_for_model("KV-X530")` resolves to `KV-X500`
- `device_range_catalog_for_model("KV-3000/5000(XYM)")` stays on the explicit XYM column
- `client.read_device_range_catalog().await?` resolves from the PLC `?K` model code

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
| VM | 10 | 0-9499 | 0-49999 | 0-63999 | 0-589823 | - |
| VB | 16 | 0-1FFF | 0-3FFF | 0-F9FF | 0-F9FF | - |
| Z | 10 | Z1-12 | Z1-12 | Z1-12 | Z1-12 | - |
| CTH | 10 | 0-3 | 0-1 | - | - | - |
| CTC | 10 | 0-7 | 0-3 | - | - | - |
| AT | 10 | - | 0-7 | 0-7 | 0-7 | - |

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
| VM | 10 | 0-9499 | 0-49999 | 0-63999 | 0-589823 | - |
| VB | 16 | 0-1FFF | 0-3FFF | 0-F9FF | 0-F9FF | - |
| Z | 10 | Z1-12 | Z1-12 | Z1-12 | Z1-12 | - |
| CTH | 10 | 0-3 | 0-3 | - | - | - |
| CTC | 10 | 0-7 | 0-3 | - | - | - |
| AT | 10 | - | 0-7 | 0-7 | 0-7 | - |

## Notes

- XYM columns may remap the same logical row to alias devices such as `X`, `Y`,
  `D`, `E`, `F`, `M`, and `L`.
- The crate keeps unsupported rows in the catalog and marks them with
  `supported = false`.
- If the catalog changes, update both `src/device_ranges.rs` and this document together.

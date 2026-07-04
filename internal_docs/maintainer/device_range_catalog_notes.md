# Device Range Catalog Notes

These notes document source cleanup applied before publishing the embedded Host Link range catalog. They are maintainer notes, not user-facing operating instructions.

## Source Corrections

The original source sheet contained a few obvious typos. The embedded catalog and public tables already reflect the corrected values.

| Location | Original value | Corrected value | Reason |
| --- | --- | --- | --- |
| `CR` row, `KV-3000(XYM)` / `KV-5000(XYM)` | `CR0000-153915` | `CR0000-CR3915` | The missing `CR` prefix is inconsistent with the same row and profile. |
| `CM` row, `KV-NANO` | `CR0000-CR8999` | `CM0000-CM8999` | The row is `CM`, so the `CR` device prefix was treated as a typo. |
| `CM` row, `KV-NANO(XYM)` | `CR0000-CR8999` | `CM0000-CM8999` | Same typo as the standard `KV-NANO` column. |
| `FM` row, `KV-3000(XYM)` / `KV-5000(XYM)` | `E0-32767` | `F0-32767` | The row is `FM`, so the `E` alias was treated as a typo and corrected to `F`. |
| `VM` row | `0-...` | `VM0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `VB` row | `0-...` | `VB0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `CTH` row | `0-...` | `CTH0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `CTC` row | `0-...` | `CTC0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |
| `AT` row | `0-...` | `AT0-...` | Bare numeric ranges were normalized to keep the device prefix in the published catalog. |


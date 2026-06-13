# Supported registers

The source of truth is the embedded catalog in `src/device_ranges.rs` plus the address parser in `src/address.rs`.

## Word device families

| Device | Category | Default type | Notes |
| --- | --- | --- | --- |
| `DM` | Word | `U` | Data memory. XYM profiles expose alias `D`. |
| `EM` | Word | `U` | Extended data memory. Not supported on every model. XYM profiles expose alias `E`. |
| `FM` | Word | `U` | File memory. Not supported on every model. XYM profiles expose alias `F`. |
| `ZF` | File refresh | `U` | File register range. Not supported on every model. |
| `W` | Word | `U` | Hex-numbered word device. |
| `TM` | Word | `U` | Temporary memory word device. |
| `CM` | Word | `U` | Control memory. |
| `VM` | Word | `U` | Variable memory. Not supported on KV-X500 profiles. |
| `Z` | Index | `D` | Native 32-bit index register. |
| `T`, `TC`, `TS` | Timer/counter | `D` | Timer-related values. `T` is in the range catalog; `TC` and `TS` are accepted as input addresses. |
| `C`, `CC`, `CS` | Timer/counter | `D` | Counter-related values. `C` is in the range catalog; `CC` and `CS` are accepted as input addresses. |
| `AT` | Timer/counter | `D` | Digital trimmer values on KV-3000/5000, KV-7000, and KV-8000 profiles. |

## Bit device families

| Device | Category | Notation | Notes |
| --- | --- | --- | --- |
| `R` | Bit | Decimal bank plus two decimal bit digits | Standard relay row. |
| `B` | Bit | Hexadecimal | Bit device with hexadecimal numbering. |
| `MR` | Bit | Decimal bank plus two decimal bit digits | Internal relay row. |
| `LR` | Bit | Decimal bank plus two decimal bit digits | Latch relay row. |
| `CR` | Bit | Decimal bank plus two decimal bit digits | Control relay row. |
| `VB` | Bit | Hexadecimal | Bit device with hexadecimal numbering. Not supported on KV-X500 profiles. |
| `X`, `Y` | Bit alias | Decimal bank plus one hexadecimal bit digit | XYM aliases published under the `R` row. |
| `M` | Bit alias | Decimal | XYM alias published under the `MR` row. |
| `L` | Bit alias | Decimal | XYM alias published under the `LR` row. |

## Type suffixes

| Form | Value kind | Notes |
| --- | --- | --- |
| Plain | Device default | Direct bit devices return bools, word devices default to `U`, and native timer/counter devices default to `D`. |
| `:U` | Unsigned 16-bit | One word. |
| `:S` | Signed 16-bit | One word. |
| `:D` | Unsigned 32-bit | Two words for word devices; one native point for `T`, `C`, `Z`, and `AT`. |
| `:L` | Signed 32-bit | Two words for word devices; one native point for `T`, `C`, `Z`, and `AT`. |
| `:F` | 32-bit float | Two words. |
| `.n` | Bit in word | One hexadecimal bit index from `0` through `F`. |

## Addressing notes

| Topic | Rule |
| --- | --- |
| `X` and `Y` notation | Use decimal bank digits followed by one hex bit digit, such as `X10F`. Do not write the whole value as one decimal number like `X275`. |
| `R`, `MR`, `LR`, and `CR` notation | Use two decimal bit digits for the low bit position, such as `R200`, `MR115`, or `CR7915`. The low two digits must be `00` through `15`. |
| `AT` restriction | `AT` exists only on KV-3000/5000, KV-7000, and KV-8000 catalog profiles. It is not in the write-device table, so write helpers reject it. |
| Catalog-only rows | `CTH` and `CTC` appear in the range catalog but are not accepted by the current address parser. |
| Default port | Use Host Link port `8501` unless your PLC configuration says otherwise. |

For model-specific ranges, see [PROFILES.md](PROFILES.md).

# Gotchas

## Timer/counter preset write fails

Only KV-8000/7000-series support preset writes. Other models error out.

Fix: use `read_timer_counter` for status only on unsupported models.

```rust
let value = client.read_timer_counter("T0").await?;
println!("{:?}", value);
```

## AT device fails on some models

AT trimmer is not available on KV-X500.

Fix: check the device range catalog before accessing `AT`.

```rust
let catalog = client.read_device_range_catalog().await?;
println!("{:?}", catalog.entry("AT"));
```

## X or Y address rejected

`X` and `Y` use decimal-bank plus hex-bit notation.

Fix: use `"X10F"` instead of `"X275"`.

```rust
let value = client.read_typed("X10F", "").await?;
println!("{:?}", value);
```

## R/MR/LR/CR address rejected

The low two digits are a decimal bit position and must be `00` through `15`.

Fix: use `"R200"` instead of hex-only or single-decimal-bit notation.

```rust
let value = client.read_typed("R200", "").await?;
println!("{:?}", value);
```

## Connection fails immediately

Default port is `8501`.

Fix: `HostLinkConnectionOptions::new("192.168.250.100")` uses `8501` by default; verify you did not override it to `1025`.

```rust
let options = HostLinkConnectionOptions::new("192.168.250.100");
println!("{:?}", options.port);
```

## DM100.D reads the wrong thing

Dot notation means bit-in-word, so `DM100.D` is bit `13`, not a double-word read.

Fix: use colon notation for data types.

```rust
let value = client.read_typed("DM100", "D").await?;
println!("{:?}", value);
```

## CTH or CTC address rejected

`CTH` and `CTC` appear in the catalog for some profiles, but the current parser does not accept them as input addresses.

Fix: treat them as catalog metadata only.

```rust
let catalog = client.read_device_range_catalog().await?;
println!("{:?}", catalog.entry("CTH"));
```

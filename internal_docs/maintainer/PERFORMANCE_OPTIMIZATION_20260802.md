# HostLink performance optimization acceptance record (2026-08-02)

## PERF2-001 — Incremental TCP receive framing

Target contract: the logical TCP session owns a growable receive accumulator and scan cursor, scans only newly available bytes, and returns an independently owned completed response.

Acceptance evidence:

- [x] A 65,536-byte body read one byte at a time has linear scan/copy counters.
- [x] Existing protocol-error and connection-retirement behavior remains covered.

## PERF2-005 — Transport-specific lazy buffers

Target contract: construction allocates no transport receive buffers; accepted TCP open allocates TCP accumulator/read buffers only, accepted UDP open allocates the UDP buffer only, repeated operations reuse them, and explicit close releases their capacity. Allocation failure occurs before DNS, socket work, or send.

Acceptance evidence:

- [x] Unit tests verify zero constructor capacity, transport separation, session reuse, and zero capacity after close.
- [x] Allocation uses fallible reserve before endpoint resolution and transport creation.

No public API, wire request, request count, or supported behavior changed. User/API documentation needs no migration update. Live PLC verification is not required because deterministic local-socket/unit tests cover the internal allocation and framing changes.

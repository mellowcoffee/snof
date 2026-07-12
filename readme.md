### ❄️ snof

*snof* is a unique ID generator. Loosely based on Snowflake IDs, *snof*
generates 64-bit identifiers composed of a 42-bit millisecond-precision
timestamp and a 22-bit sequence that distinguishes identifiers generated within
the same millisecond. Timestamps are measured from a fixed epoch of Jan 01
2026, giving the timestamp field a range of roughly 139 years.

The generator tracks its entire state in a single atomic word, making ID
generation thread-safe and lock-free. When the per-millisecond sequence is
exhausted, or the system clock moves backwards, the generator spins until
validity is restored.

#### Layout

```text
 63                    22 21          0
+------------------------+-------------+
|   timestamp (42 bits)  | seq (22)    |
+------------------------+-------------+
```

#### Usage

```rust
use std::sync::Arc;
use std::thread;

use snof::SnowflakeGenerator;

let generator = Arc::new(SnowflakeGenerator::new());

let threads: Vec<_> = (0..4)
    .map(|_| {
        let g = Arc::clone(&generator);
        thread::spawn(move || println!("{}", g.generate()))
    })
    .collect();

for t in threads {
    t.join().unwrap();
}
```

##### Storing in a database

Snowflakes are `u64`, but many stores (for example Postgres `bigint`) accept
only signed 64-bit integers. Use `to_i64` / `from_i64`, which perform a
lossless bitwise reinterpret:

```rust
let id = generator.generate();
let stored: i64 = id.to_i64();
assert_eq!(snof::Snowflake::from_i64(stored), id);
```

##### Inspecting an ID

```rust
let id = generator.generate();
let _ms: u64 = id.extract_unix_timestamp(); // UNIX ms, epoch-adjusted
let _seq: u64 = id.sequence();
let _raw: u64 = id.raw();
```

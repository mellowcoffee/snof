## ❄️ snof

*snof* is a unique ID generator. Loosely based on Snowflake IDs, *snof*
generates 64-bit identifiers composed of a 42-bit millisecond-precision
timestamp and a 22-bit sequence that distinguishes identifiers generated within
the same millisecond. Timestamps are measured from a fixed epoch of Jan 01
2026, giving the timestamp field a range of roughly 139 years.

```text
 63                    22 21          0
+------------------------+-------------+
|   timestamp (42 bits)  | seq (22)    |
+------------------------+-------------+
```

The generator tracks its entire state in a single atomic word, making ID
generation thread-safe and lock-free. When the per-millisecond sequence is
exhausted, or the system clock moves backwards, the generator spins until
validity is restored.

> [!NOTE]
> snof is under development, breaking changes could be made to the API before
> `1.0.0`. Please pin the version if you intend to use snof in a production
> environment.

### Usage

For detailed documentation, please refer to the [documentation](https://docs.rs/snof/latest/snof/).

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

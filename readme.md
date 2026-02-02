### ❄️ snof

*snof* is a unique ID generator. Loosely based on snowflake ID-s, *snof*
generates 64 bit long identifiers consisting of a 32 bit millisecond-based
timestamp, and 22 bits of sequence distinguishing identifiers generated within
the same millisecond.

The generator uses atomic operations for tracking state, thus it provides a
thread-safe, lock-free way of generating unique ID-s. In case of the sequence
being exhausted, or the clock moving backwards, the generator spins until
validity is restored.

#### Usage

```rust
use snof::SnowflakeGenerator;

fn main() {
    let generator = Arc::new(SnowflakeGenerator::new());

    let threads: Vec<_> = (0..4).map(|_| {
        let other_generator = Arc::clone(&generator);
        thread::spawn(move || {
            let id = other_generator.generate();
            println!("thread id: {}", id.0);
        })
    }).collect();

    for t in threads { t.join().unwrap(); }
}
```

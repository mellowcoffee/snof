//! ### ❄️ snof
//!
//! *snof* is a unique ID generator. Loosely based on Snowflake IDs, *snof*
//! generates 64-bit identifiers composed of a 42-bit millisecond-precision
//! timestamp and a 22-bit sequence that distinguishes identifiers generated within
//! the same millisecond. Timestamps are measured from a fixed epoch of Jan 01
//! 2026, giving the timestamp field a range of roughly 139 years.
//!
//! The generator tracks its entire state in a single atomic word, making ID
//! generation thread-safe and lock-free. When the per-millisecond sequence is
//! exhausted, or the system clock moves backwards, the generator spins until
//! validity is restored.
//!
//! #### Layout
//!
//! ```text
//!  63                    22 21          0
//! +------------------------+-------------+
//! |   timestamp (42 bits)  | seq (22)    |
//! +------------------------+-------------+
//! ```
//!
//! #### Usage
//!
//! ```rust
//! use std::sync::Arc;
//! use std::thread;
//!
//! use snof::SnowflakeGenerator;
//!
//! let generator = Arc::new(SnowflakeGenerator::new());
//!
//! let threads: Vec<_> = (0..4)
//!     .map(|_| {
//!         let g = Arc::clone(&generator);
//!         thread::spawn(move || println!("{}", g.generate()))
//!     })
//!     .collect();
//!
//! for t in threads {
//!     t.join().unwrap();
//! }
//! ```

use std::cmp::Ordering;
use std::fmt;
use std::hint::spin_loop;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unix timestamp of Jan 01 2026 00:00:00 GMT+0000 in milliseconds.
const EPOCH: u64 = 1_767_225_600_000;
/// Out of the 64-bits of the identifier, the last 22 are reserved for the sequence.
const SEQUENCE_BITS: u32 = 22;
/// Mask for extracting the sequence bits.
const SEQUENCE_MASK: u64 = (1 << SEQUENCE_BITS) - 1;

/// A thread-safe, lock-free Snowflake generator.
///
/// Initialize with [`SnowflakeGenerator::new()`].
#[derive(Debug)]
pub struct SnowflakeGenerator {
    /// Last generated snowflake.
    last_state: AtomicU64,
}

impl SnowflakeGenerator {
    /// Initializes a new [`SnowflakeGenerator`].
    ///
    /// The initial state is set to the current time with sequence 0.
    #[must_use]
    pub fn new() -> Self {
        let now_ts = epoch_relative_now();
        Self {
            last_state: AtomicU64::new(now_ts << SEQUENCE_BITS),
        }
    }

    /// Generates a [`Snowflake`].
    ///
    /// Utilizes a CAS loop using atomics. If the sequence is exhausted or the clock moves
    /// backwards, it spins until validity is restored.
    pub fn generate(&self) -> Snowflake {
        let mut current_bits = self.last_state.load(AtomicOrdering::Relaxed);

        loop {
            let last_ts = current_bits >> SEQUENCE_BITS;
            let last_seq = current_bits & SEQUENCE_MASK;

            let now_ts = epoch_relative_now();

            let next_bits = match now_ts.cmp(&last_ts) {
                Ordering::Greater => now_ts << SEQUENCE_BITS,
                Ordering::Equal => {
                    let next_seq = last_seq + 1;
                    if next_seq > SEQUENCE_MASK {
                        spin_loop();
                        current_bits = self.last_state.load(AtomicOrdering::Relaxed);
                        continue;
                    }
                    (last_ts << SEQUENCE_BITS) | next_seq
                }
                Ordering::Less => {
                    spin_loop();
                    current_bits = self.last_state.load(AtomicOrdering::Relaxed);
                    continue;
                }
            };

            match self.last_state.compare_exchange_weak(
                current_bits,
                next_bits,
                AtomicOrdering::SeqCst,
                AtomicOrdering::Relaxed,
            ) {
                Ok(_) => return Snowflake(next_bits),
                Err(fresh_bits) => current_bits = fresh_bits,
            }
        }
    }
}

impl Default for SnowflakeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// [`Snowflake`] wrapper.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Snowflake(pub u64);

impl Snowflake {
    /// Extract the millisecond-based UNIX timestamp of a [`Snowflake`].
    ///
    /// The resulting timestamp is relative to the UNIX epoch, Jan 01 1970 00:00:00 GMT+0000
    #[must_use]
    pub const fn extract_unix_timestamp(&self) -> u64 {
        (self.0 >> SEQUENCE_BITS) + EPOCH
    }

    /// Extract the sequence component of a [`Snowflake`].
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0 & SEQUENCE_MASK
    }

    /// Access the raw 64-bit value of a [`Snowflake`].
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Reinterpret a [`Snowflake`] as a signed 64-bit integer.
    ///
    /// The conversion is a bitwise reinterpretation; it is lossless and reversed by
    /// [`Snowflake::from_i64`].
    #[allow(clippy::cast_possible_wrap)]
    #[must_use]
    pub const fn to_i64(self) -> i64 {
        self.0 as i64
    }

    /// Reconstruct a [`Snowflake`] from the signed representation produced by
    /// [`Snowflake::to_i64`].
    #[allow(clippy::cast_sign_loss)]
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        Self(value as u64)
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Snowflake {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>().map(Snowflake)
    }
}

impl From<Snowflake> for u64 {
    fn from(value: Snowflake) -> Self {
        value.0
    }
}

impl From<u64> for Snowflake {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Gets the current millisecond-based UNIX timestamp.
///
/// # Panics
///
/// Panics if the current UNIX timestamp exceeds u64 capacity.
#[must_use]
pub fn unix_timestamp_now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis(),
    )
    .expect("Timestamp exceeds u64 capacity")
}

/// Current UNIX timestamp in milliseconds, relative to [`EPOCH`].
fn epoch_relative_now() -> u64 {
    unix_timestamp_now_ms().saturating_sub(EPOCH)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    use super::*;

    #[test]
    fn extract_recovers_epoch_relative_timestamp() {
        // sequence 0, timestamp component = 5 ms past EPOCH
        let sf = Snowflake(5 << SEQUENCE_BITS);
        assert_eq!(sf.extract_unix_timestamp(), EPOCH + 5);
        assert_eq!(sf.sequence(), 0);
    }

    #[test]
    fn sequence_field_is_masked_off() {
        let sf = Snowflake((7 << SEQUENCE_BITS) | 42);
        assert_eq!(sf.sequence(), 42);
        assert_eq!(sf.extract_unix_timestamp(), EPOCH + 7);
    }

    #[test]
    fn i64_roundtrip_is_lossless() {
        for raw in [0u64, 1, SEQUENCE_MASK, 1 << 41, u64::MAX] {
            let sf = Snowflake(raw);
            assert_eq!(Snowflake::from_i64(sf.to_i64()), sf);
        }
    }

    #[test]
    fn display_fromstr_roundtrip() {
        let sf = Snowflake(123_456_789);
        let parsed: Snowflake = sf.to_string().parse().unwrap();
        assert_eq!(parsed, sf);
    }

    #[test]
    fn ordering_matches_raw_value() {
        assert!(Snowflake(1) < Snowflake(2));
        assert!(Snowflake(1 << SEQUENCE_BITS) > Snowflake(SEQUENCE_MASK));
    }

    #[test]
    fn single_thread_ids_strictly_increase() {
        let g = SnowflakeGenerator::new();
        let mut prev = g.generate();
        for _ in 0..10_000 {
            let next = g.generate();
            assert!(next > prev, "{next:?} !> {prev:?}");
            prev = next;
        }
    }

    #[test]
    fn concurrent_ids_are_unique() {
        let g = Arc::new(SnowflakeGenerator::new());
        const THREADS: usize = 8;
        const PER_THREAD: usize = 20_000;

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let g = Arc::clone(&g);
                thread::spawn(move || (0..PER_THREAD).map(|_| g.generate().0).collect::<Vec<_>>())
            })
            .collect();

        let mut all = HashSet::with_capacity(THREADS * PER_THREAD);
        for h in handles {
            for id in h.join().unwrap() {
                assert!(all.insert(id), "duplicate id {id}");
            }
        }
        assert_eq!(all.len(), THREADS * PER_THREAD);
    }

    #[test]
    fn sequence_increments_within_same_millisecond() {
        // Two IDs generated back-to-back either share a timestamp with
        // increasing sequence, or land in a later millisecond.
        let g = SnowflakeGenerator::new();
        let a = g.generate();
        let b = g.generate();
        if a.extract_unix_timestamp() == b.extract_unix_timestamp() {
            assert_eq!(b.sequence(), a.sequence() + 1);
        } else {
            assert!(b.extract_unix_timestamp() > a.extract_unix_timestamp());
        }
    }
}

//! ### ❄️ snof
//! 
//! *snof* is a unique ID generator. Loosely based on snowflake ID-s, *snof*
//! generates 64 bit long identifiers consisting of a 32 bit millisecond-based
//! timestamp, and 22 bits of sequence distinguishing identifiers generated within
//! the same millisecond.
//! 
//! The generator uses atomic operations for tracking state, thus it provides a
//! thread-safe, lock-free way of generating unique ID-s. In case of the sequence
//! being exhausted, or the clock moving backwards, the generator spins until
//! validity is restored.
//! 
//! #### Usage
//! 
//! ```rust
//! use snof::SnowflakeGenerator;
//! 
//! fn main() {
//!     let generator = Arc::new(SnowflakeGenerator::new());
//! 
//!     let threads: Vec<_> = (0..4).map(|_| {
//!         let other_generator = Arc::clone(&generator);
//!         thread::spawn(move || {
//!             let id = other_generator.generate();
//!             println!("thread id: {}", id.0);
//!         })
//!     }).collect();
//! 
//!     for t in threads { t.join().unwrap(); }
//! }
//! ```

use std::cmp::Ordering;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unix timestamp of Jan 01 2026 00:00:00 GMT+0000 in milliseconds.
const EPOCH: u128 = 1_767_225_600_000;
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
    pub fn new() -> Self {
        let now_ms = unix_timestamp_now_ms();
        let initial_id = Snowflake::new(now_ms, 0);
        Self {
            last_state: AtomicU64::new(initial_id.0),
        }
    }

    /// Generates a [`Snowflake`].
    ///
    /// Utilizes a CAS loop using atomics. If the sequence is exhausted or the clock moves backwards, it spins until validity is restored.
    pub fn generate(&self) -> Snowflake {
        let mut current_bits = self.last_state.load(AtomicOrdering::Relaxed);

        loop {
            let last_ts = current_bits >> SEQUENCE_BITS;
            let last_seq = current_bits & SEQUENCE_MASK;
            
            let now_ms = unix_timestamp_now_ms();
            let now_ts = u64::try_from(now_ms.saturating_sub(EPOCH))
                .expect("Timestamp exceeds u64 capacity");

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

/// [`Snowflake`] wrapper.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snowflake(pub u64);

impl Snowflake {
    /// Construct a [`Snowflake`] from timestamp and sequence.
    fn new(timestamp: u128, sequence: u64) -> Self {
        let shifted = u64::try_from(timestamp - EPOCH).expect("Timestamp overflow") << SEQUENCE_BITS;
        Snowflake(shifted | (sequence & SEQUENCE_MASK))
    }

    /// Extract the millisecond-based UNIX timestamp of a [`Snowflake`].
    ///
    /// The resulting timestamp is relative to the UNIX epoch, Jan 01 1970 00:00:00 GMT+0000
    pub fn extract_unix_timestamp(&self) -> u128 {
        u128::from(self.0 >> SEQUENCE_BITS) + EPOCH
    }
}

impl From<Snowflake> for u64 {
    fn from(value: Snowflake) -> Self {
        value.0
    }
}

/// Gets the current millisecond-based UNIX timestamp.
pub fn unix_timestamp_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

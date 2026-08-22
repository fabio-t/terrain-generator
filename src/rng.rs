//! A small, dependency-free PCG32 random number generator.
//!
//! The generator is fully deterministic for a given seed on every platform,
//! which keeps terrain generation reproducible from Rust, from the CLI and
//! from the Elixir bindings alike.

/// A PCG32 (XSH-RR) pseudo-random number generator.
///
/// See <https://www.pcg-random.org> for the algorithm. This is *not* a
/// cryptographic generator; it is used for droplet placement during erosion.
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

const MULTIPLIER: u64 = 6364136223846793005;

impl Pcg32 {
    /// Creates a generator from a seed and a stream identifier.
    ///
    /// Different streams with the same seed produce independent sequences.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Returns the next random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Returns a uniformly distributed `f32` in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_seed() {
        let mut a = Pcg32::new(42, 1);
        let mut b = Pcg32::new(42, 1);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn streams_differ() {
        let mut a = Pcg32::new(42, 1);
        let mut b = Pcg32::new(42, 2);
        let same = (0..100).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 3);
    }

    #[test]
    fn floats_in_unit_range() {
        let mut rng = Pcg32::new(7, 3);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }
}

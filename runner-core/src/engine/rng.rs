/// Small deterministic xorshift32 PRNG. Not suitable for anything but gameplay.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng { state: 0x9E37_79B9 };
        rng.mix(seed);
        rng
    }

    /// Folds extra entropy (e.g. a timer value) into the state.
    pub fn mix(&mut self, value: u64) {
        self.state ^= (value as u32) ^ ((value >> 32) as u32).rotate_left(13);
        if self.state == 0 {
            self.state = 0x9E37_79B9;
        }
        self.next_u32();
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Uniform-ish integer in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        debug_assert!(lo <= hi);
        lo + self.next_u32() % (hi - lo + 1)
    }

    /// Float in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(1234);
        let mut b = Rng::new(1234);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..100).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 5);
    }

    #[test]
    fn zero_seed_is_not_stuck() {
        let mut r = Rng::new(0);
        assert_ne!(r.next_u32(), 0);
        let mut r = Rng::new(0x9E37_79B9);
        assert_ne!(r.next_u32(), r.next_u32());
    }

    #[test]
    fn range_and_unit_stay_in_bounds() {
        let mut r = Rng::new(42);
        for _ in 0..10_000 {
            let v = r.range(10, 18);
            assert!((10..=18).contains(&v));
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }
}

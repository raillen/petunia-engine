/// Deterministic pseudo-random generator: **mulberry32**, ported exactly from the Digi-Tactics
/// `rng.ts` so that a differential test against the TypeScript reference can compare outputs
/// bit for bit. Determinism-critical code is ours, never a dependency's.
///
/// The sequence for a given seed must never change. Golden values in the tests below were
/// produced by running the original JavaScript algorithm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// Any integer is accepted; only its low 32 bits matter (same as JavaScript's `seed | 0`).
    pub fn new(seed: i64) -> Self {
        Self { state: seed as u32 }
    }

    /// Float in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let state = self.state;
        let mut t = (state ^ (state >> 15)).wrapping_mul(1 | state);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    /// Integer in `[min_inclusive, max_exclusive)`. Returns `min_inclusive` for an empty range.
    pub fn int(&mut self, min_inclusive: i64, max_exclusive: i64) -> i64 {
        if max_exclusive <= min_inclusive {
            return min_inclusive;
        }
        let span = (max_exclusive - min_inclusive) as f64;
        min_inclusive + (self.next_f64() * span).floor() as i64
    }

    /// True with the given probability (`0.15` means 15%).
    pub fn chance(&mut self, probability: f64) -> bool {
        if probability <= 0.0 {
            return false;
        }
        if probability >= 1.0 {
            return true;
        }
        self.next_f64() < probability
    }

    /// A random element, or `None` for an empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let index = self.int(0, items.len() as i64) as usize;
        items.get(index)
    }

    /// Shuffled copy (Fisher-Yates); the original slice is untouched.
    pub fn shuffle<T: Clone>(&mut self, items: &[T]) -> Vec<T> {
        let mut copy = items.to_vec();
        for i in (1..copy.len()).rev() {
            let j = self.int(0, i as i64 + 1) as usize;
            copy.swap(i, j);
        }
        copy
    }

    /// Independent generator derived from this one. The parent is not advanced.
    pub fn fork(&self, salt: i32) -> Rng {
        Rng {
            state: self.state ^ (salt as u32).wrapping_mul(0x9e37_79b9),
        }
    }

    /// Internal state, for snapshots and reproducing failures.
    pub fn state(&self) -> u32 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_javascript_reference_next() {
        let mut rng = Rng::new(20261022);
        let expected = [
            0.5591592639684677,
            0.21883013984188437,
            0.5584872781764716,
            0.36965543031692505,
            0.12802476389333606,
        ];
        for value in expected {
            assert_eq!(rng.next_f64(), value);
        }
    }

    #[test]
    fn matches_the_javascript_reference_int() {
        let mut rng = Rng::new(20261022);
        let drawn: Vec<i64> = (0..5).map(|_| rng.int(0, 100)).collect();
        assert_eq!(drawn, [55, 21, 55, 36, 12]);
    }

    #[test]
    fn negative_seed_uses_the_low_32_bits() {
        let mut rng = Rng::new(-1);
        let expected = [0.8964226141106337, 0.189478256739676, 0.7156526781618595];
        for value in expected {
            assert_eq!(rng.next_f64(), value);
        }
    }

    #[test]
    fn fork_does_not_advance_the_parent() {
        let parent = Rng::new(7);
        let mut child = parent.fork(3);
        assert_eq!(parent.state(), 7);
        let expected = [0.8436362366192043, 0.9787938336376101, 0.8029511275235564];
        for value in expected {
            assert_eq!(child.next_f64(), value);
        }
    }

    #[test]
    fn shuffle_matches_the_javascript_reference() {
        let mut rng = Rng::new(42);
        let items: Vec<i32> = (0..8).collect();
        assert_eq!(rng.shuffle(&items), [2, 7, 1, 0, 6, 5, 3, 4]);
    }

    #[test]
    fn pick_on_empty_slice_is_none() {
        let mut rng = Rng::new(1);
        let empty: [i32; 0] = [];
        assert_eq!(rng.pick(&empty), None);
    }
}

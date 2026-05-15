use std::ops::{Deref, DerefMut};

use rand::{
    distr::{Bernoulli, Uniform},
    prelude::*,
};
use rand_pcg::Pcg64;

pub struct RandomEngine {
    rng: Pcg64,
    will_send_msg_distr: Bernoulli,
    destination_distr: Uniform<usize>,
}

impl Deref for RandomEngine {
    type Target = Pcg64;
    fn deref(&self) -> &Self::Target {
        &self.rng
    }
}

impl DerefMut for RandomEngine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rng
    }
}

impl RandomEngine {
    pub fn new(seed: Option<u64>, rho: f64, v: usize) -> Option<Self> {
        if v == 0 || rho < 0.0 || rho > 1.0 {
            return None;
        }
        let rng = seed.map_or_else(|| Pcg64::try_from_os_rng().unwrap(), Pcg64::seed_from_u64);
        let will_send_msg_distr = Bernoulli::new(rho).unwrap();
        let destination_distr = Uniform::new(0, v - 1).unwrap();
        Some(RandomEngine {
            rng,
            will_send_msg_distr,
            destination_distr,
        })
    }
    pub fn shuffle_slice<T>(&mut self, items: &mut [T]) {
        items.shuffle(&mut self.rng);
    }

    pub fn choose_from<T: Clone>(&mut self, items: &[T]) -> T {
        items
            .choose(&mut self.rng)
            .expect("The slice is non-empty")
            .clone()
    }

    pub fn sample_will_send_msg(&mut self) -> bool {
        self.will_send_msg_distr.sample(&mut self.rng)
    }

    pub fn sample_destination(&mut self, vertex_to_exclude: usize) -> usize {
        let x = self.destination_distr.sample(&mut self.rng);
        if x >= vertex_to_exclude { x + 1 } else { x }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_new_with_seed() {
        let seed = 42;
        let _engine = RandomEngine::new(Some(seed), 0.5, 10).unwrap();
    }

    #[test]
    fn test_random_new_without_seed() {
        let _engine = RandomEngine::new(None, 0.5, 10).unwrap();
    }

    #[test]
    fn test_random_invalid_new_call() {
        assert!(RandomEngine::new(Some(0), 0.5, 0).is_none());
        assert!(RandomEngine::new(Some(0), 5.0, 0).is_none());
        assert!(RandomEngine::new(Some(0), -5.0, 0).is_none());
    }

    #[test]
    fn test_random_choose_from() {
        let mut engine = RandomEngine::new(Some(1), 0.5, 10).unwrap();
        let items = vec![1, 2, 3, 4, 5];
        let chosen = engine.choose_from(&items);
        assert!(items.contains(&chosen));
    }
    #[test]
    fn test_deref_and_mut_deref_example() {
        use rand::Rng;
        #[inline(never)]
        fn dummy_deref(_: &impl Rng) {}
        #[inline(never)]
        fn dummy_mut_deref(_: &mut impl Rng) {}
        let engine = RandomEngine::new(Some(1), 0.5, 10).unwrap();
        dummy_deref(engine.deref());
        let mut engine = RandomEngine::new(Some(1), 0.5, 10).unwrap();
        dummy_mut_deref(engine.deref_mut());
    }
    #[test]
    #[should_panic(expected = "The slice is non-empty")]
    fn test_random_choose_from_empty() {
        let mut engine = RandomEngine::new(Some(1), 0.5, 10).unwrap();
        let items: Vec<i32> = vec![];
        engine.choose_from(&items);
    }

    #[test]
    fn test_random_sample_will_send_msg() {
        let mut engine_always = RandomEngine::new(Some(3), 1.0, 10).unwrap();
        for _ in 0..100 {
            assert!(engine_always.sample_will_send_msg());
        }

        let mut engine_never = RandomEngine::new(Some(4), 0.0, 10).unwrap();
        for _ in 0..100 {
            assert!(!engine_never.sample_will_send_msg());
        }
    }

    #[test]
    fn test_random_sample_destination() {
        let v = 10;
        let mut engine = RandomEngine::new(Some(5), 0.5, v).unwrap();
        let exclude = 5;

        for _ in 0..100 {
            let dest = engine.sample_destination(exclude);
            assert!(dest < v);
            assert_ne!(dest, exclude);
        }
    }

    #[test]
    fn test_shuffle_slice() {
        let mut engine = RandomEngine::new(Some(12345), 0.5, 10).unwrap();
        let mut items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let original = items.clone();

        engine.shuffle_slice(&mut items);

        assert_eq!(items.len(), original.len());
        for item in &original {
            assert!(items.contains(item));
        }
    }
}

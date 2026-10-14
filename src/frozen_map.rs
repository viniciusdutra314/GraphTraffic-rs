pub struct FrozenMap<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K: Ord, V> FrozenMap<K, V> {
    /// Build from entries whose keys are strictly increasing (sorted and unique).
    /// # Panics
    /// Panics if keys are out of order or repeated.
    pub fn from_sorted_entries(entries: impl IntoIterator<Item = (K, V)>) -> Self {
        let (keys, values): (Vec<K>, Vec<V>) = entries.into_iter().unzip();
        assert!(
            keys.windows(2).all(|pair| pair[0] < pair[1]),
            "FrozenMap keys must be strictly increasing"
        );
        Self { keys, values }
    }

    /// Return the associated value in O(log number of keys), or None if absent.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.keys
            .binary_search(key)
            .ok()
            .map(|index| &self.values[index])
    }
}

#[cfg(test)]
mod tests {
    use super::FrozenMap;
    #[test]
    fn lookup_preserves_the_association_with_each_key() {
        let map = FrozenMap::from_sorted_entries([
            (2, String::from("last")),
            (7, String::from("first")),
            (11, String::from("middle")),
        ]);
        assert_eq!(map.get(&2).map(String::as_str), Some("last"));
        assert_eq!(map.get(&7).map(String::as_str), Some("first"));
        assert_eq!(map.get(&11).map(String::as_str), Some("middle"));
    }

    #[test]
    fn absent_keys_return_none_before_between_and_after_entries() {
        let map = FrozenMap::from_sorted_entries([(2, 20), (7, 70)]);
        for missing in [1, 5, 8] {
            assert_eq!(
                map.get(&missing),
                None
            );
        }
    }

    #[test]
    fn empty_map_returns_none() {
        let map = FrozenMap::<usize, usize>::from_sorted_entries([]);
        assert_eq!(map.get(&0), None);
    }

    #[test]
    fn single_entry_can_be_found() {
        let map = FrozenMap::from_sorted_entries([(3, 42)]);
        assert_eq!(map.get(&3), Some(&42));
        assert_eq!(map.get(&4), None);
    }

    #[test]
    #[should_panic(expected = "FrozenMap keys must be strictly increasing")]
    fn construction_rejects_unsorted_keys() {
        FrozenMap::from_sorted_entries([(7, 70), (2, 20)]);
    }

    #[test]
    #[should_panic(expected = "FrozenMap keys must be strictly increasing")]
    fn construction_rejects_duplicate_keys() {
        FrozenMap::from_sorted_entries([(2, 20), (2, 30)]);
    }
}

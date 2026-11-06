use super::Observer;
use crate::graph_dynamics::{Edge, Vertex};
use std::collections::HashMap;
use std::num::NonZero;

pub struct ObserverEdgeCapacity {
    update_interval: NonZero<u64>,
    capacities_over_time: HashMap<u64, Vec<usize>>,
}

impl ObserverEdgeCapacity {
    pub fn new(update_interval: NonZero<u64>) -> Self {
        return ObserverEdgeCapacity {
            update_interval,
            capacities_over_time: HashMap::new(),
        };
    }
}

impl Observer for ObserverEdgeCapacity {
    fn measure(&mut self, iteration: u64, edges: &[Edge], _vertices: &[Vertex]) {
        if iteration % self.update_interval == 0 {
            self.capacities_over_time
                .insert(iteration, edges.iter().map(|e| e.capacity()).collect());
        }
    }
    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group) {
        let group = hdf5_group.create_group("ObserverEdgeCapacity").unwrap();
        let mut iters: Vec<u64> = self.capacities_over_time.keys().copied().collect();
        iters.sort_unstable();

        for it in iters {
            let time_group = group.create_group(&it.to_string()).unwrap();
            let capacities = &self.capacities_over_time[&it];
            let values: Vec<u64> = capacities.iter().map(|&c| c as u64).collect();
            time_group
                .new_dataset_builder()
                .with_data(&values)
                .create("capacities")
                .ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_capacity_records_iterations_on_measure() {
        let mut observer = ObserverEdgeCapacity::new(NonZero::new(2).unwrap());
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        for i in 0..5 {
            observer.measure(i, &edges, &vertices);
        }

        assert_eq!(observer.capacities_over_time.len(), 3);
        assert!(observer.capacities_over_time.contains_key(&0));
        assert!(observer.capacities_over_time.contains_key(&2));
        assert!(observer.capacities_over_time.contains_key(&4));
    }
}

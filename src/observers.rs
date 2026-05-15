use petgraph::visit::EdgeCount;

use crate::graph_dynamics::{Edge, Vertex};
use crate::graph_structure::PreComputedGraph;
use crate::schema::SimulationConfigurationItemObserversItem as ConfigObserverEnum;
use std::collections::{HashMap};
use std::num::NonZero;

pub trait Observer {
    fn measure(&mut self, iteration: u64, edges: &[Edge], vertices: &[Vertex]);
    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group);
}

pub fn create_observer(config: &ConfigObserverEnum, graph: &PreComputedGraph) -> Box<dyn Observer> {
    match config {
        ConfigObserverEnum::ObserverEdgeCapacity(update_interval) => {
            Box::new(ObserverEdgeCapacity::new(*update_interval))
        }
        ConfigObserverEnum::ObserverEdgeQueue => {
            Box::new(ObserverEdgeQueue::new(graph.edge_count()))
        }
        ConfigObserverEnum::ObserverEdgeReceivedMessages => {
            Box::new(ObserverEdgeReceivedMessages::new(graph.edge_count()))
        }
        ConfigObserverEnum::ObserverTotalMessages => Box::new(ObserverTotalMessages::default()),
    }
}

pub struct ObserverEdgeQueue {
    pub queue_histograms: Vec<HashMap<usize, usize>>,
}

impl ObserverEdgeQueue {
    pub fn new(num_edges: usize) -> Self {
        return ObserverEdgeQueue {
            queue_histograms: vec![HashMap::new(); num_edges],
        };
    }
}

impl Observer for ObserverEdgeQueue {
    fn measure(&mut self, _iteration: u64, edges: &[Edge], _vertices: &[Vertex]) {
        for (edge_id, edge_object) in edges.iter().enumerate() {
            let num_messages = edge_object.queue_size();
            *self.queue_histograms[edge_id]
                .entry(num_messages)
                .or_insert(0) += 1;
        }
    }
    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group) {
        let group = hdf5_group.create_group("ObserverEdgeQueue").unwrap();
        for (i, hist) in self.queue_histograms.iter().enumerate() {
            let edge_group = group.create_group(&i.to_string()).unwrap();
            let keys: Vec<u64> = hist.keys().map(|&k| k as u64).collect();
            let values: Vec<u64> = hist.values().map(|&v| v as u64).collect();
            edge_group
                .new_dataset_builder()
                .with_data(&keys)
                .create("keys")
                .ok();
            edge_group
                .new_dataset_builder()
                .with_data(&values)
                .create("values")
                .ok();
        }
    }
}

pub struct ObserverEdgeReceivedMessages {
    received_histograms: Vec<HashMap<u64, u64>>,
}

impl ObserverEdgeReceivedMessages {
    pub fn new(num_edges: usize) -> Self {
        ObserverEdgeReceivedMessages {
            received_histograms: vec![HashMap::new(); num_edges],
        }
    }
}

impl Observer for ObserverEdgeReceivedMessages {
    fn measure(&mut self, _iteration: u64, edges: &[Edge], _vertices: &[Vertex]) {
        for (edge_id, edge_object) in edges.iter().enumerate() {
            let num_messages = edge_object.total_num_messages_processed();
            *self.received_histograms[edge_id]
                .entry(num_messages)
                .or_insert(0) += 1;
        }
    }
    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group) {
        let group = hdf5_group
            .create_group("ObserverEdgeReceivedMessages")
            .unwrap();
        for (i, hist) in self.received_histograms.iter().enumerate() {
            let edge_group = group.create_group(&i.to_string()).unwrap();
            let keys: Vec<u64> = hist.keys().map(|&k| k as u64).collect();
            let values: Vec<u64> = hist.values().map(|&v| v as u64).collect();
            edge_group
                .new_dataset_builder()
                .with_data(&keys)
                .create("keys")
                .ok();
            edge_group
                .new_dataset_builder()
                .with_data(&values)
                .create("values")
                .ok();
        }
    }
}

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

#[derive(Default)]
pub struct ObserverTotalMessages {
    total_messages_over_time: Vec<u64>,
}

impl Observer for ObserverTotalMessages {
    fn measure(&mut self, _iteration: u64, edges: &[Edge], _vertices: &[Vertex]) {
        self.total_messages_over_time
            .push(edges.iter().map(|e| e.queue_size() as u64).sum());
    }
    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group) {
        hdf5_group
            .new_dataset_builder()
            .with_data(&self.total_messages_over_time)
            .create("ObserverTotalMessages")
            .ok();
    }
}

#[cfg(test)]
mod observer_behavior_tests {
    use super::*;

    #[test]
    fn total_messages_records_iterations() {
        let mut observer = ObserverTotalMessages::default();
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        for i in 0..3 {
            observer.measure(i, &edges, &vertices);
        }

        assert_eq!(observer.total_messages_over_time.len(), 3);
        assert!(observer.total_messages_over_time.iter().all(|&v| v == 0));
    }

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

    #[test]
    fn edge_queue_handles_empty_edges() {
        let mut observer = ObserverEdgeQueue::new(0);
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        observer.measure(0, &edges, &vertices);

        assert_eq!(observer.queue_histograms.len(), 0);
    }

    #[test]
    fn edge_received_messages_handles_empty_edges() {
        let mut observer = ObserverEdgeReceivedMessages::new(0);
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        observer.measure(0, &edges, &vertices);

        assert_eq!(observer.received_histograms.len(), 0);
    }
}

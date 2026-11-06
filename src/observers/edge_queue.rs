use super::Observer;
use crate::graph_dynamics::{Edge, Vertex};
use std::collections::BTreeMap;

pub struct ObserverEdgeQueue {
    pub queue_histograms: Vec<BTreeMap<usize, usize>>,
}

impl ObserverEdgeQueue {
    pub fn new(num_edges: usize) -> Self {
        return ObserverEdgeQueue {
            queue_histograms: vec![BTreeMap::new(); num_edges],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_queue_handles_empty_edges() {
        let mut observer = ObserverEdgeQueue::new(0);
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        observer.measure(0, &edges, &vertices);

        assert_eq!(observer.queue_histograms.len(), 0);
    }

    #[test]
    fn edge_queue_counts_samples_in_increasing_size_order() {
        let mut observer = ObserverEdgeQueue::new(2);
        for (iteration, queue_size) in [5, 0, 2, 5].into_iter().enumerate() {
            let mut edges = [Edge::new(1).unwrap(), Edge::new(1).unwrap()];
            for _ in 0..queue_size {
                edges[0].add_message(crate::graph_dynamics::Message::new(0, 1, 0).unwrap(), false);
            }
            observer.measure(iteration as u64, &edges, &[]);
        }
        assert_eq!(
            observer.queue_histograms[0]
                .iter()
                .map(|(&size, &count)| (size, count))
                .collect::<Vec<_>>(),
            [(0, 1), (2, 1), (5, 2)]
        );
    }
}

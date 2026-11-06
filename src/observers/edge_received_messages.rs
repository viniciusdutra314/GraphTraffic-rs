use super::Observer;
use crate::graph_dynamics::{Edge, Vertex};
use std::collections::HashMap;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_received_messages_handles_empty_edges() {
        let mut observer = ObserverEdgeReceivedMessages::new(0);
        let edges = Vec::<Edge>::new();
        let vertices = Vec::<Vertex>::new();

        observer.measure(0, &edges, &vertices);

        assert_eq!(observer.received_histograms.len(), 0);
    }
}

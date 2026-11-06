use super::Observer;
use crate::graph_dynamics::{Edge, Vertex};

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
mod tests {
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
}

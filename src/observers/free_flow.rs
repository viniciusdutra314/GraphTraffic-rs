use super::Observer;
use crate::graph_dynamics::{Edge, Vertex};

#[derive(hdf5_metno::H5Type, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
struct FreeFlowRecord {
    num_samples: u64,
    free_flow_times: u64,
}

pub struct ObserverFreeFlowRate {
    records: Vec<FreeFlowRecord>,
}

impl ObserverFreeFlowRate {
    pub fn new(num_edges: usize) -> Self {
        Self {
            records: vec![FreeFlowRecord::default(); num_edges],
        }
    }
}

impl Observer for ObserverFreeFlowRate {
    fn measure(&mut self, _iteration: u64, edges: &[Edge], _vertices: &[Vertex]) {
        for (edge,record) in edges.iter().zip(self.records.iter_mut()) {
            record.num_samples += 1;
            if edge.queue_size() <= edge.capacity() {
                record.free_flow_times += 1;
            }
        }
    }

    fn save(self: Box<Self>, hdf5_group: &hdf5_metno::Group) {
        hdf5_group
            .new_dataset_builder()
            .with_data(&self.records)
            .create("ObserverFreeFlowRate")
            .expect("Could not save ObserverFreeFlowRate");
    }
}

#[cfg(test)]
mod free_flow_tests {
    use super::*;
    use crate::graph_dynamics::Message;

    fn edge(capacity: usize, queue_size: usize) -> Edge {
        let mut edge = Edge::new(capacity).unwrap();
        for _ in 0..queue_size {
            edge.add_message(Message::new(0, 1, 0).unwrap(), false);
        }
        edge
    }

    fn counts(samples: u64, free: u64) -> FreeFlowRecord {
        FreeFlowRecord {
            num_samples: samples,
            free_flow_times: free,
        }
    }

    #[test]
    fn measures_edges_independently_using_current_capacity() {
        let mut observer = ObserverFreeFlowRate::new(2);

        // The empty queue is free; the queue with 2 messages exceeds capacity 1.
        let edges = [edge(3, 0), edge(1, 2)];
        observer.measure(0, &edges, &[]);
        assert_eq!(observer.records, [counts(1, 1), counts(1, 0)]);

        // Both queues are free: 2 is below capacity 3 and equals capacity 2.
        let edges = [edge(3, 2), edge(2, 2)];
        observer.measure(1, &edges, &[]);
        assert_eq!(observer.records, [counts(2, 2), counts(2, 1)]);

        // The first queue equals capacity 3; the second exceeds capacity 1.
        let edges = [edge(3, 3), edge(1, 2)];
        observer.measure(2, &edges, &[]);
        assert_eq!(observer.records, [counts(3, 3), counts(3, 1)]);

        // The first queue exceeds capacity 3; the second equals capacity 2.
        // The free counts accumulate independently for each edge.
        let edges = [edge(3, 4), edge(2, 2)];
        observer.measure(3, &edges, &[]);
        assert_eq!(observer.records, [counts(4, 3), counts(4, 2)]);
    }
}

use crate::graph_structure::PreComputedGraph;
use crate::random_engine::RandomEngine;
use crate::schema::RoutingMethod;
use core::panic;
use std::collections::VecDeque;

#[derive(hdf5_metno::H5Type, Debug, Clone, Default)]
#[repr(C)]
pub struct Vertex {
    num_arrived_msgs: u64,
    num_traveling_messages: u64,
    total_distance: u64,
    total_traveling_time: u64,
    num_messages_generated: u64,
}

impl Vertex {
    #[allow(dead_code)]
    pub fn num_arrived_msgs(&self) -> u64 {
        self.num_arrived_msgs
    }
    #[allow(dead_code)]
    pub fn num_traveling_msgs(&self) -> u64 {
        self.num_traveling_messages
    }
    #[allow(dead_code)]
    pub fn total_distance(&self) -> u64 {
        self.total_distance
    }
    #[allow(dead_code)]
    pub fn total_traveling_time(&self) -> u64 {
        self.total_traveling_time
    }
    #[allow(dead_code)]
    pub fn num_messages_generated(&self) -> u64 {
        self.num_messages_generated
    }

    pub fn increment_num_traveling_msgs(&mut self) {
        self.num_traveling_messages += 1;
    }

    pub fn increment_num_created_messages(&mut self) {
        self.num_messages_generated += 1;
    }
    pub fn update_statistics(
        &mut self,
        message: &Message,
        ideal_distance: u16,
        ready_to_observe: bool,
    ) {
        if let MessageState::Arrived { time_arrived } = message.state {
            if ready_to_observe {
                self.num_arrived_msgs += 1;
                let incremented_travalled_time =
                    time_arrived.checked_sub(message.time_creation()).unwrap();
                if incremented_travalled_time == 0 {
                    panic!("Traveling time cannot be zero when updating statistics.");
                }
                self.total_traveling_time += incremented_travalled_time;
                self.total_distance += ideal_distance as u64;
            }
        } else {
            panic!("Message has not arrived it should be not added to the statistics")
        }
    }
}

#[derive(Debug, Clone)]
pub struct Edge {
    queue: VecDeque<Message>,
    capacity: usize,
    total_num_messages_processed: u64,
}

#[derive(hdf5_metno::H5Type)]
#[repr(C)]
pub struct HDF5Edge {
    capacity: usize,
    total_num_messages_processed: u64,
}

impl Edge {
    pub fn new(capacity: usize) -> Result<Self, &'static str> {
        if capacity == 0 {
            return Err("Edge capacity must be at least 1.");
        }
        Ok(Self {
            queue: VecDeque::new(),
            total_num_messages_processed: 0,
            capacity: capacity,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn total_num_messages_processed(&self) -> u64 {
        self.total_num_messages_processed
    }

    pub fn to_hdf5(&self) -> HDF5Edge {
        HDF5Edge {
            capacity: self.capacity,
            total_num_messages_processed: self.total_num_messages_processed,
        }
    }
    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = std::cmp::max(1, capacity);
    }

    pub fn add_message(&mut self, message: Message, ready_to_observe: bool) {
        if ready_to_observe {
            self.total_num_messages_processed += 1
        };
        self.queue.push_back(message);
    }

    pub fn messages_to_deliver_mut(&mut self) -> impl Iterator<Item = Message> {
        self.queue
            .drain(0..std::cmp::min(self.capacity, self.queue.len()))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MessageState {
    Created,
    InEdge { edge: (usize, usize) },
    Arrived { time_arrived: u64 },
}

#[derive(Clone, Copy, Debug)]
pub struct Message {
    source: usize,
    destination: usize,
    time_creation: u64,
    state: MessageState,
}
impl Message {
    pub fn new(source: usize, destination: usize, create_time: u64) -> Option<Self> {
        if source == destination {
            return None;
        }
        Some(Self {
            source,
            destination,
            time_creation: create_time,
            state: MessageState::Created,
        })
    }
    pub fn source(&self) -> usize {
        self.source
    }
    pub fn destination(&self) -> usize {
        self.destination
    }
    pub fn time_creation(&self) -> u64 {
        self.time_creation
    }
    pub fn state(&self) -> MessageState {
        self.state
    }

    pub fn step(
        &mut self,
        graph: &PreComputedGraph,
        rng: &mut RandomEngine,
        routing_method: RoutingMethod,
        current_time: u64,
    ) {
        let current_vertex = match self.state() {
            MessageState::Arrived { .. } => panic!("Message already arrived"),
            MessageState::Created => self.source,
            MessageState::InEdge { edge: (_, to) } => to,
        };

        if current_vertex == self.destination {
            self.state = MessageState::Arrived {
                time_arrived: current_time,
            };
        } else {
            let next_vertex =
                self.calculate_next_vertex(current_vertex, &graph, rng, routing_method);

            self.state = MessageState::InEdge {
                edge: (current_vertex, next_vertex),
            };
        }
    }

    fn calculate_next_vertex(
        &self,
        current_vertex: usize,
        graph: &PreComputedGraph,
        rng: &mut RandomEngine,
        routing_method: RoutingMethod,
    ) -> usize {
        let dist = graph.get_distance(current_vertex, self.destination());

        let next_vertex = match routing_method {
            RoutingMethod::RandomWalk => rng.choose_from(graph.neighbors_slice(current_vertex)),
            RoutingMethod::LimitedVisibility(visibility) if (dist as u64) > visibility => {
                rng.choose_from(graph.neighbors_slice(current_vertex))
            }
            RoutingMethod::MinimalPaths | RoutingMethod::LimitedVisibility(_) => {
                graph.sample_closer_neighbor(current_vertex, self.destination, rng)
            }
        };
        assert_ne!(
            next_vertex, current_vertex,
            "Next vertex cannot be the same as current vertex"
        );
        next_vertex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_edge_new() {
        let edge = Edge::new(10);
        assert!(edge.is_ok());
        let edge = edge.unwrap();
        assert_eq!(edge.capacity(), 10);
        assert_eq!(edge.queue_size(), 0);
        assert_eq!(edge.total_num_messages_processed(), 0);

        let edge = Edge::new(0);
        assert!(edge.is_err());
    }

    #[test]
    fn teste_edge_set_capacity() {
        let mut edge = Edge::new(10).unwrap();
        edge.set_capacity(20);
        assert_eq!(edge.capacity(), 20);

        edge.set_capacity(0);
        assert_eq!(edge.capacity(), 1);
    }

    #[test]
    fn test_edge_add_message() {
        let mut edge = Edge::new(10).unwrap();
        let msg = Message::new(0, 1, 0).unwrap();
        edge.add_message(msg,true);

        assert_eq!(edge.queue_size(), 1);
        assert_eq!(edge.total_num_messages_processed(), 1);
    }
    #[test]
    fn test_message_source_to_destination() {
        assert!(Message::new(0, 0, 0).is_none())
    }
    #[test]
    #[should_panic]
    fn test_step_message_already_arrived() {
        let mut msg = Message::new(0, 1, 0).unwrap();
        msg.state = MessageState::Arrived { time_arrived: 3 };
        msg.step(
            &dummy_graph_cache(),
            &mut dummy_random_generator(),
            RoutingMethod::MinimalPaths,
            0,
        );
    }
    #[test]
    fn test_edge_messages_to_deliver() {
        let mut edge = Edge::new(2).unwrap();
        let msg1 = Message::new(0, 1, 0).unwrap();
        let msg2 = Message::new(0, 1, 1).unwrap();
        let msg3 = Message::new(0, 1, 2).unwrap();

        edge.add_message(msg1,true);
        edge.add_message(msg2,true);
        edge.add_message(msg3,true);

        assert_eq!(edge.queue_size(), 3);

        let delivered: Vec<_> = edge.messages_to_deliver_mut().collect();
        assert_eq!(delivered.len(), 2);
        assert_eq!(edge.queue_size(), 1);

        let delivered: Vec<_> = edge.messages_to_deliver_mut().collect();
        assert_eq!(delivered.len(), 1);
        assert_eq!(edge.queue_size(), 0);

        let delivered: Vec<_> = edge.messages_to_deliver_mut().collect();
        assert_eq!(delivered.len(), 0);
    }

    #[test]
    fn test_vertex_new() {
        let vertex = Vertex::default();
        assert_eq!(vertex.num_arrived_msgs(), 0);
        assert_eq!(vertex.num_traveling_msgs(), 0);
        assert_eq!(vertex.total_distance(), 0);
        assert_eq!(vertex.total_traveling_time(), 0);
        assert_eq!(vertex.num_messages_generated(), 0);
    }

    #[test]
    fn test_vertex_increment_num_traveling_msgs() {
        let mut vertex = Vertex::default();
        vertex.increment_num_traveling_msgs();
        assert_eq!(vertex.num_traveling_msgs(), 1);
        vertex.increment_num_traveling_msgs();
        assert_eq!(vertex.num_traveling_msgs(), 2);
    }

    #[test]
    fn test_vertex_increment_num_created_messages() {
        let mut vertex = Vertex::default();
        vertex.increment_num_created_messages();
        assert_eq!(vertex.num_messages_generated(), 1);
        vertex.increment_num_created_messages();
        assert_eq!(vertex.num_messages_generated(), 2);
    }

    #[test]
    #[should_panic]
    fn test_instantenous_message_to_update_statistics() {
        let mut vertex = Vertex::default();
        let mut msg = Message::new(0, 1, 10).unwrap();
        msg.state = MessageState::Arrived { time_arrived: 10 };
        vertex.update_statistics(&msg, 10,true);
    }

    #[test]
    #[should_panic]
    fn test_travelling_message_to_update_statistics() {
        let mut vertex = Vertex::default();
        let mut msg = Message::new(0, 1, 10).unwrap();
        msg.state = MessageState::InEdge { edge: ((3, 5)) };
        vertex.update_statistics(&msg, 15,true);
    }

    #[test]
    fn test_vertex_update_statistics() {
        let mut vertex = Vertex::default();

        let mut msg = Message::new(0, 1, 10).unwrap();
        msg.state = MessageState::Arrived { time_arrived: 20 };

        let ideal_distance = 5;
        vertex.update_statistics(&msg, ideal_distance,true);

        assert_eq!(vertex.num_arrived_msgs(), 1);
        assert_eq!(vertex.total_traveling_time(), 10); // 20 - 10
        assert_eq!(vertex.total_distance(), 5);

        assert_eq!(vertex.num_arrived_msgs(), 1);
        assert_eq!(vertex.total_traveling_time(), 10);
        assert_eq!(vertex.total_distance(), 5);

        let mut msg2 = Message::new(0, 1, 5).unwrap();
        msg2.state = MessageState::Arrived { time_arrived: 15 };
        let ideal_distance2 = 8;

        vertex.update_statistics(&msg2, ideal_distance2,true);

        assert_eq!(vertex.num_arrived_msgs(), 2);
        assert_eq!(vertex.total_traveling_time(), 20);
        assert_eq!(vertex.total_distance(), 13);
    }
}

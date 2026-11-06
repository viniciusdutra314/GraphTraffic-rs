mod edge_capacity;
mod edge_queue;
mod edge_received_messages;
mod free_flow;
mod total_messages;

pub use edge_capacity::ObserverEdgeCapacity;
pub use edge_queue::ObserverEdgeQueue;
pub use edge_received_messages::ObserverEdgeReceivedMessages;
pub use free_flow::ObserverFreeFlowRate;
pub use total_messages::ObserverTotalMessages;

use crate::graph_dynamics::{Edge, Vertex};
use crate::graph_structure::PreComputedGraph;
use crate::schema::SimulationConfigurationItemObserversItem as ConfigObserverEnum;

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
        ConfigObserverEnum::ObserverFreeFlowRate => {
            Box::new(ObserverFreeFlowRate::new(graph.edge_count()))
        }
        ConfigObserverEnum::ObserverTotalMessages => Box::new(ObserverTotalMessages::default()),
    }
}

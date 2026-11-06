mod edge_capacity;
pub use edge_capacity::ModifierEdgeCapacity;

use crate::graph_dynamics::{Edge, Vertex};

pub trait Modifier {
    fn should_stop_simulation(&self) -> bool;
    fn measure_and_check_if_ready_to_act(
        &mut self,
        iteration: u64,
        edges: &[Edge],
        vertices: &[Vertex],
    ) -> bool;
    fn act(&mut self, edges: &mut [Edge], vertices: &mut [Vertex]);
}

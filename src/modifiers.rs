use petgraph::visit::EdgeCount;
use std::cmp;

use crate::graph_dynamics::{Edge, Vertex};
use crate::graph_structure::PreComputedGraph;
use crate::observers::{Observer, ObserverEdgeQueue};
use crate::schema::ModifierEdgeCapacity as ConfigModifierEdgeCapacity;

pub fn create_modifiers(
    config: &ConfigModifierEdgeCapacity,
    graph: &PreComputedGraph,
) -> Box<dyn Modifier> {
    return Box::new(ModifierEdgeCapacity::new(config.clone(), graph));
}

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

pub struct ModifierEdgeCapacity {
    inner_sensor: ObserverEdgeQueue,
    free_flow_rate: f64,
    free_flow_sampling_time: u64,
    minimal_capacity: u64,
    multiplier: f64,
}

impl ModifierEdgeCapacity {
    pub fn new(config: ConfigModifierEdgeCapacity, graph: &PreComputedGraph) -> Self {
        return ModifierEdgeCapacity {
            inner_sensor: ObserverEdgeQueue::new(graph.edge_count()),
            free_flow_rate: config.free_flow_rate,
            free_flow_sampling_time: config.free_flow_sampling_time,
            minimal_capacity: config.minimal_capacity,
            multiplier: config.multiplier,
        };
    }
}

impl Modifier for ModifierEdgeCapacity {
    fn measure_and_check_if_ready_to_act(
        &mut self,
        iteration: u64,
        edges: &[Edge],
        vertices: &[Vertex],
    ) -> bool {
        self.inner_sensor.measure(iteration, edges, vertices);
        return iteration > 0 && iteration % self.free_flow_sampling_time == 0;
    }

    fn should_stop_simulation(&self) -> bool {
        return false;
    }

    fn act(&mut self, edges: &mut [Edge], _vertices: &mut [Vertex]) {
        for (index, edge) in edges.iter_mut().enumerate() {
            let histogram = &mut self.inner_sensor.queue_histograms[index];
            let mut cumulative_probability = 0.0;
            for (&queue_size, &occurrences) in histogram.iter() {
                cumulative_probability +=
                    (occurrences as f64) / (self.free_flow_sampling_time as f64);

                if cumulative_probability >= self.free_flow_rate {
                    edge.set_capacity(cmp::max(
                        self.minimal_capacity as usize,
                        f64::ceil(queue_size as f64 * self.multiplier) as usize,
                    ));
                    break;
                }
            }
            histogram.clear();
        }
    }
}

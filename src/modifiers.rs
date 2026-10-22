use std::cmp;
use std::hint::cold_path;

use crate::graph_dynamics::{Edge, Vertex};
use crate::schema::ModifierEdgeCapacity as ConfigModifierEdgeCapacity;

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
    free_flow_rate: f64,
    free_flow_sampling_time: u64,
    momentum:f64,
    past_histograms:Vec<Vec<f64>>,
    current_histograms:Vec<Vec<f64>>,
}

impl ModifierEdgeCapacity {
    pub fn new(config: ConfigModifierEdgeCapacity) -> Self {
        return ModifierEdgeCapacity {
            free_flow_rate: config.free_flow_rate,
            free_flow_sampling_time: config.free_flow_sampling_time,
            momentum:config.momentum,
            past_histograms:vec![],
            current_histograms:vec![]
        };
    }
}

impl Modifier for ModifierEdgeCapacity {
    fn measure_and_check_if_ready_to_act(
        &mut self,
        iteration: u64,
        edges: &[Edge],
        _vertices: &[Vertex],
    ) -> bool {
        for (edge_id, edge_object) in edges.iter().enumerate() {
            let current_histogram=&mut self.current_histograms[edge_id];
            let num_messages = edge_object.queue_size();
            if num_messages >= current_histogram.len(){
                current_histogram.resize(num_messages+1, 0.0);
            } 
            current_histogram[num_messages]+=1.0 / (self.free_flow_sampling_time as f64);
        }
        return iteration > 0 && iteration % self.free_flow_sampling_time == 0;
    }

    fn should_stop_simulation(&self) -> bool {
        return false;
    }

    fn act(&mut self, edges: &mut [Edge], _vertices: &mut [Vertex]) {
        for (edge_id, edge) in edges.iter_mut().enumerate() {
            let current_hist=&self.current_histograms[edge_id];
            let  past_hist=&mut self.past_histograms[edge_id];
            if past_hist.is_empty(){
                cold_path();
                *past_hist=current_hist.clone();
            }
            past_hist.resize(std::cmp::max(past_hist.len(),current_hist.len()), 0.0);
            let mut estimated_capacity=0;
            let mut cumulative_prob=0.0;
            for i in 0..past_hist.len(){
                past_hist[i]=(1.0-self.momentum)*(current_hist.get(i).unwrap_or(&0.0))+past_hist[i]*self.momentum;
                cumulative_prob+=past_hist[i];
                if cumulative_prob<=self.free_flow_rate{
                    estimated_capacity+=1;
                }
            }
            let new_capacity = cmp::max(1, estimated_capacity);
            let old_capacity = edge.capacity();
            let geom_mean = f64::sqrt((new_capacity * old_capacity) as f64).ceil() as usize;
            edge.set_capacity(geom_mean);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn median_modifier(num_edges: usize) -> ModifierEdgeCapacity {
        ModifierEdgeCapacity {
            inner_sensor: ObserverEdgeQueue::new(num_edges),
            free_flow_rate: 0.5,
            free_flow_sampling_time: 4,
        }
    }

    #[test]
    fn quantile_accumulates_queue_sizes_from_smallest_to_largest() {
        let mut modifier = median_modifier(1);
        modifier.free_flow_sampling_time = 8;
        let histogram = &mut modifier.inner_sensor.queue_histograms[0];
        // Insert large sizes first: the sorted observations are
        // [1, 1, 4, 4, 4, 4, 9, 9], so the median queue size is 4.
        histogram.insert(9, 2);
        histogram.insert(4, 4);
        histogram.insert(1, 2);
        let mut edges = [Edge::new(4).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            4,
            "The CDF reaches 50% at queue size 4; sqrt(4 * 4) keeps capacity at 4"
        );
    }

    #[test]
    fn quantile_accepts_probability_exactly_equal_to_the_requested_rate() {
        let mut modifier = median_modifier(1);
        // Half of the observations have size 1; the other half have size 9.
        modifier.inner_sensor.queue_histograms[0].extend([(1, 2), (9, 2)]);
        let mut edges = [Edge::new(4).unwrap()];
        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            2,
            "The first bin already reaches 50%, so use size 1: sqrt(4 * 1) = 2"
        );
    }

    #[test]
    fn requested_rate_selects_the_upper_quartile_instead_of_the_median() {
        let mut modifier = median_modifier(1);
        modifier.free_flow_rate = 0.75;
        modifier.inner_sensor.queue_histograms[0].extend([(1, 1), (4, 1), (9, 1), (16, 1)]);
        let mut edges = [Edge::new(4).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            6,
            "75% of the four samples have size at most 9, so use sqrt(4 * 9) = 6; the median would give 4"
        );
    }

    #[test]
    fn capacity_decreases_using_the_geometric_mean() {
        let mut modifier = median_modifier(1);
        modifier.inner_sensor.queue_histograms[0].insert(4, 4);
        let mut edges = [Edge::new(9).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(edges[0].capacity(), 6, "sqrt(9 * 4) = 6");
    }

    #[test]
    fn capacity_rounds_the_geometric_mean_up() {
        let mut modifier = median_modifier(1);
        modifier.inner_sensor.queue_histograms[0].insert(2, 4);
        let mut edges = [Edge::new(4).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            3,
            "sqrt(4 * 2) is 2sqrt(2)≃2.8; round up to 3"
        );
    }

    #[test]
    fn zero_queue_quantile_uses_one_before_the_geometric_mean() {
        let mut modifier = median_modifier(1);
        modifier.inner_sensor.queue_histograms[0].insert(0, 4);
        let mut edges = [Edge::new(9).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            3,
            "Clamp queue size 0 to 1 before smoothing: sqrt(9 * 1) = 3, rather than dropping directly to capacity 1"
        );
    }

    #[test]
    fn each_edge_uses_its_own_histogram() {
        let mut modifier = median_modifier(2);
        modifier.inner_sensor.queue_histograms[0].insert(1, 4);
        modifier.inner_sensor.queue_histograms[1].insert(9, 4);
        let mut edges = [Edge::new(4).unwrap(), Edge::new(4).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(edges[0].capacity(), 2, "First edge: sqrt(4 * 1) = 2");
        assert_eq!(edges[1].capacity(), 6, "Second edge: sqrt(4 * 9) = 6");
    }

    #[test]
    fn adaptation_discards_samples_before_the_next_window() {
        let mut modifier = median_modifier(1);
        let mut edges = [Edge::new(8).unwrap()];

        modifier.inner_sensor.queue_histograms[0].insert(2, 4);
        modifier.act(&mut edges, &mut []);

        assert_eq!(edges[0].capacity(), 4, "First window: sqrt(8 * 2) = 4");
        assert!(
            modifier.inner_sensor.queue_histograms[0].is_empty(),
            "The completed sampling window must be cleared after adaptation"
        );

        modifier.inner_sensor.queue_histograms[0].insert(8, 4);
        modifier.act(&mut edges, &mut []);

        assert_eq!(
            edges[0].capacity(),
            6,
            "Only the new size-8 samples count: ceil(sqrt(4 * 8)) = 6; stale size-2 samples would give 3"
        );
    }

    #[test]
    fn no_samples_leave_capacity_unchanged() {
        let mut modifier = median_modifier(1);
        let mut edges = [Edge::new(7).unwrap()];

        modifier.act(&mut edges, &mut []);

        assert_eq!(edges[0].capacity(), 7);
    }

    #[test]
    fn measurement_records_every_iteration_and_adapts_at_sampling_multiples() {
        let mut modifier = median_modifier(1);
        modifier.free_flow_sampling_time = 3;
        let edges = [Edge::new(1).unwrap()];

        for (iteration, should_adapt) in [
            (0, false),
            (1, false),
            (2, false),
            (3, true),
            (4, false),
            (5, false),
            (6, true),
        ] {
            assert_eq!(
                modifier.measure_and_check_if_ready_to_act(iteration, &edges, &[]),
                should_adapt,
                "With sampling time 3, adapt at iterations 3 and 6, but never at iteration 0 (got iteration {iteration})"
            );
        }

        assert_eq!(
            modifier.inner_sensor.queue_histograms[0].get(&0),
            Some(&7),
            "All seven iterations must be measured, including those that do not request adaptation"
        );
    }
}

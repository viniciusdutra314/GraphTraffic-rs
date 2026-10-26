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
    momentum: f64,
    past_histograms: Vec<Vec<f64>>,
    current_histograms: Vec<Vec<f64>>,
}

impl ModifierEdgeCapacity {
    pub fn new(config: ConfigModifierEdgeCapacity, num_edges: usize) -> Self {
        return ModifierEdgeCapacity {
            free_flow_rate: config.free_flow_rate,
            free_flow_sampling_time: config.free_flow_sampling_time,
            momentum: config.momentum,
            past_histograms: vec![vec![]; num_edges],
            current_histograms: vec![vec![]; num_edges],
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
            let current_histogram = &mut self.current_histograms[edge_id];
            let num_messages = edge_object.queue_size();
            if num_messages >= current_histogram.len() {
                current_histogram.resize(num_messages + 1, 0.0);
            }
            current_histogram[num_messages] += 1.0 / (self.free_flow_sampling_time as f64);
        }
        return iteration > 0 && iteration % self.free_flow_sampling_time == 0;
    }

    fn should_stop_simulation(&self) -> bool {
        return false;
    }

    fn act(&mut self, edges: &mut [Edge], _vertices: &mut [Vertex]) {
        for (edge_id, edge) in edges.iter_mut().enumerate() {
            let current_hist = &mut self.current_histograms[edge_id];
            let past_hist = &mut self.past_histograms[edge_id];
            if past_hist.is_empty() {
                cold_path();
                *past_hist = current_hist.clone();
            }
            past_hist.resize(std::cmp::max(past_hist.len(), current_hist.len()), 0.0);
            let mut estimated_capacity = 0;
            let mut cumulative_prob = 0.0;
            for i in 0..past_hist.len() {
                past_hist[i] = (1.0 - self.momentum) * (current_hist.get(i).unwrap_or(&0.0))
                    + past_hist[i] * self.momentum;
                cumulative_prob += past_hist[i];
                if cumulative_prob < self.free_flow_rate {
                    estimated_capacity += 1;
                }
            }
            let new_capacity = cmp::max(1, estimated_capacity);
            let old_capacity = edge.capacity();
            let geom_mean = f64::sqrt((new_capacity * old_capacity) as f64).ceil() as usize;
            edge.set_capacity(geom_mean);
            current_hist.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_dynamics::Message;

    fn modifier(num_edges: usize, momentum: f64) -> ModifierEdgeCapacity {
        let config = serde_json::from_value(serde_json::json!({
            "type": "ModifierEdgeCapacity",
            "free_flow_rate": 0.5,
            "free_flow_sampling_time": 4,
            "momentum": momentum
        }))
        .unwrap();
        ModifierEdgeCapacity::new(config, num_edges)
    }

    #[track_caller]
    fn assert_distribution(actual: &[f64], expected: &[f64]) {
        const TOLERANCE: f64 = 1.0e-12;
        assert_eq!(actual.len(), expected.len(), "histogram support changed");
        for (q, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() <= TOLERANCE,
                "q={q}: expected {expected}, got {actual}"
            );
        }
        assert!(
            (actual.iter().sum::<f64>() - 1.0).abs() <= TOLERANCE,
            "histogram is not normalized: {actual:?}"
        );
    }
    #[test]
    fn first_window_initializes_history_independently_of_momentum() {
        for momentum in [0.0, 0.25, 0.5, 0.75, 1.0 - f64::from(f32::EPSILON)] {
            let mut modifier = modifier(1, momentum);
            modifier.current_histograms[0] = vec![0.0, 0.5, 0.5];
            modifier.act(&mut [Edge::new(4).unwrap()], &mut []);
            assert_distribution(&modifier.past_histograms[0], &[0.0, 0.5, 0.5]);
        }
    }

    #[test]
    fn momentum_weights_history_across_equal_growing_and_shrinking_support() {
        // alpha = momentum: H_new = alpha * H_old + (1 - alpha) * P_current.
        // Missing bins have probability zero; vectors below are padded accordingly.
        for (name, momentum, past, current, expected) in [
            // 0.25 * [1, 0] + 0.75 * [0, 1] = [0.25, 0.75].
            (
                "equal support",
                0.25,
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![0.25, 0.75],
            ),
            // 0.5 * [1, 0, 0] + 0.5 * [0, 0.5, 0.5] = [0.5, 0.25, 0.25].
            (
                "growing support",
                0.5,
                vec![1.0],
                vec![0.0, 0.5, 0.5],
                vec![0.5, 0.25, 0.25],
            ),
            // 0.5 * [0, 0, 0.5, 0, 0.5] + 0.5 * [1, 0, 0, 0, 0]
            // = [0.5, 0, 0.25, 0, 0.25], including bins past the median at q=0.
            (
                "shrinking support",
                0.5,
                vec![0.0, 0.0, 0.5, 0.0, 0.5],
                vec![1.0],
                vec![0.5, 0.0, 0.25, 0.0, 0.25],
            ),
            // 0 * [0, 0, 0, 1] + 1 * [0, 1, 0, 0] = [0, 1, 0, 0].
            (
                "zero momentum",
                0.0,
                vec![0.0, 0.0, 0.0, 1.0],
                vec![0.0, 1.0],
                vec![0.0, 1.0, 0.0, 0.0],
            ),
            // With epsilon = f32::EPSILON:
            // (1 - epsilon) * [1, 0] + epsilon * [0, 1] = [1 - epsilon, epsilon].
            (
                "near unit momentum",
                1.0 - f64::from(f32::EPSILON),
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![1.0 - f64::from(f32::EPSILON), f64::from(f32::EPSILON)],
            ),
        ] {
            let mut modifier = modifier(1, momentum);
            modifier.past_histograms[0] = past;
            modifier.current_histograms[0] = current;
            modifier.act(&mut [Edge::new(4).unwrap()], &mut []);
            assert_eq!(modifier.past_histograms[0], expected, "{name}");
        }
    }

    #[test]
    fn consecutive_windows_keep_exponential_memory_not_just_the_last_window() {
        let mut modifier = modifier(1, 0.5);
        let mut edges = [Edge::new(4).unwrap()];
        for (current, expected) in [
            // First window: H_1 = P_1 = [1].
            (vec![1.0], vec![1.0]),
            // H_2 = 0.5 * [1, 0] + 0.5 * [0, 1] = [0.5, 0.5].
            (vec![0.0, 1.0], vec![0.5, 0.5]),
            // H_3 = 0.5 * [0.5, 0.5, 0] + 0.5 * [0, 0, 1] = [0.25, 0.25, 0.5].
            (vec![0.0, 0.0, 1.0], vec![0.25, 0.25, 0.5]),
            // H_4 = 0.5 * [0.25, 0.25, 0.5] + 0.5 * [1, 0, 0]
            // = [0.625, 0.125, 0.25].
            (vec![1.0], vec![0.625, 0.125, 0.25]),
        ] {
            modifier.current_histograms[0] = current;
            modifier.act(&mut edges, &mut []);
            assert_distribution(&modifier.past_histograms[0], &expected);
        }
    }

    #[test]
    fn quantile_selects_first_crossing_including_equality_and_zero_bins() {
        // CDF(0) = 0.25; CDF(2) = 0.25 + 0.5 = 0.75; CDF(4) = 0.75 + 0.25 = 1.
        // Select the first q with CDF(q) >= eta, then C_new = ceil(sqrt(16*max(1,q))):
        // eta in (0, 0.25] -> q=0 -> 4; (0.25, 0.75] -> q=2 -> ceil(sqrt(32))=6;
        // eta in (0.75, 1] -> q=4 -> sqrt(64)=8.
        for (eta, expected_capacity) in [
            (f64::EPSILON, 4),
            (0.25, 4),
            (0.25 + f64::EPSILON, 6),
            (0.5, 6),
            (0.75, 6),
            (0.75 + f64::EPSILON, 8),
            (1.0 - f64::EPSILON, 8),
            (1.0, 8),
        ] {
            let mut modifier = modifier(1, 0.0);
            modifier.free_flow_rate = eta;
            modifier.current_histograms[0] = vec![0.25, 0.0, 0.5, 0.0, 0.25];
            let mut edges = [Edge::new(16).unwrap()];
            modifier.act(&mut edges, &mut []);
            assert_eq!(edges[0].capacity(), expected_capacity, "eta={eta}");
        }
    }

    #[test]
    fn geometric_capacity_update_covers_increase_decrease_rounding_and_minimum() {
        // All observations are at q=queue, so the quantile is q for any eta > 0.
        // C_new is the geometric mean rounded up: ceil(sqrt(C_old * max(1, q))).
        for (old, queue, expected) in [
            (4, 9, 6), // ceil(sqrt(4*9)) = 6: increase.
            (9, 4, 6), // ceil(sqrt(9*4)) = 6: decrease.
            (4, 4, 4), // ceil(sqrt(4*4)) = 4: fixed point.
            (4, 2, 3), // ceil(sqrt(4*2)) = ceil(2.828...) = 3.
            (9, 0, 3), // q=0 is clamped to 1: ceil(sqrt(9*1)) = 3.
            (1, 0, 1), // ceil(sqrt(1*1)) = 1: minimum capacity.
        ] {
            let mut modifier = modifier(1, 0.0);
            modifier.current_histograms[0] = vec![0.0; queue + 1];
            modifier.current_histograms[0][queue] = 1.0;
            let mut edges = [Edge::new(old).unwrap()];
            modifier.act(&mut edges, &mut []);
            assert_eq!(edges[0].capacity(), expected, "old={old}, queue={queue}");
        }
    }

    #[test]
    fn measurement_windows_initialize_normalize_and_clear_per_edge_state() {
        let mut modifier = modifier(2, 0.5);
        let mut edges = [Edge::new(4).unwrap(), Edge::new(9).unwrap()];
        assert!(!modifier.should_stop_simulation());
        assert_eq!(modifier.current_histograms, vec![Vec::<f64>::new(); 2]);
        assert_eq!(modifier.past_histograms, vec![Vec::<f64>::new(); 2]);

        // Follow the production lifecycle: measure from iteration 1, then act
        // only at window boundaries. Edge 0 changes queue size; edge 1 stays idle.
        for iteration in 1..=12 {
            if iteration == 5 {
                edges[0].add_message(Message::new(0, 1, 0).unwrap(), false);
            }
            let ready = modifier.measure_and_check_if_ready_to_act(iteration, &edges, &[]);
            assert_eq!(ready, iteration % 4 == 0, "iteration={iteration}");
            if ready {
                // Four measurements, each contributing 1/4 at the observed queue:
                // first window P(0)=4*(1/4)=1; later windows P(1)=4*(1/4)=1.
                let sample: &[f64] = if iteration == 4 { &[1.0] } else { &[0.0, 1.0] };
                assert_distribution(&modifier.current_histograms[0], sample);
                assert_distribution(&modifier.current_histograms[1], &[1.0]);
                modifier.act(&mut edges, &mut []);
                let history: &[f64] = match iteration {
                    4 => &[1.0],         // First window: H = P = [1].
                    8 => &[0.5, 0.5],    // 0.5*[1, 0] + 0.5*[0, 1] = [0.5, 0.5].
                    12 => &[0.25, 0.75], // 0.5*[0.5, 0.5] + 0.5*[0, 1] = [0.25, 0.75].
                    _ => unreachable!(),
                };
                assert_distribution(&modifier.past_histograms[0], history);
                // Edge 1 always observes q=0: 0.5*[1] + 0.5*[1] = [1].
                assert_distribution(&modifier.past_histograms[1], &[1.0]);
                assert!(modifier.current_histograms.iter().all(Vec::is_empty));
            }
        }
    }
}

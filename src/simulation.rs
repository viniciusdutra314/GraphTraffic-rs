use petgraph::visit::{EdgeCount, NodeCount};

use crate::SimulationConfigurationItem;
use crate::graph_dynamics::{Edge, HDF5Edge, Message, MessageState, Vertex};
use crate::graph_structure::PreComputedGraph;
use crate::modifiers::{Modifier, create_modifiers};
use crate::observers::{Observer, create_observer};
use crate::random_engine::RandomEngine;
use std::sync::Arc;
pub struct Simulation {
    current_time_step: u64,
    graph_with_cache: Arc<PreComputedGraph>,
    config: SimulationConfigurationItem,
    random_engine: RandomEngine,
    edges: Vec<Edge>,
    vertices: Vec<Vertex>,
    observers: Vec<Box<dyn Observer>>,
    modifiers: Vec<Box<dyn Modifier>>,
    shuffled_edge_indices: Vec<usize>,
}

impl Simulation {
    pub fn new(graph_cache: Arc<PreComputedGraph>, config: &SimulationConfigurationItem) -> Self {
        let v = graph_cache.node_count();
        let e = graph_cache.edge_count();
        let seed = config.random_seed;
        let observers = config
            .observers
            .iter()
            .map(|config| create_observer(config, &graph_cache))
            .collect();
        let modifiers = config
            .modifiers
            .iter()
            .map(|config| create_modifiers(config, &graph_cache))
            .collect();
        Simulation {
            current_time_step: 0,
            graph_with_cache: graph_cache,
            config: config.clone(),
            random_engine: RandomEngine::new(seed, config.message_generation, v).unwrap(),
            edges: vec![Edge::new(1).unwrap(); e],
            vertices: vec![Vertex::default(); v],
            observers: observers,
            modifiers: modifiers,
            shuffled_edge_indices: (0..e).collect(),
        }
    }
    fn run(&mut self) {
        let warm_up = self.config.warm_up_iterations.unwrap_or(0);
        for _ in 0..self.config.max_iterations.into() {
            let ready_to_observe = self.current_time_step >= warm_up;
            self.message_generate(ready_to_observe);
            self.current_time_step += 1;
            if ready_to_observe {
                self.observers
                    .iter_mut()
                    .for_each(|o| o.measure(self.current_time_step, &self.edges, &self.vertices));
                for modifiers in self.modifiers.iter_mut() {
                    if modifiers.should_stop_simulation() {
                        return ();
                    }
                    if modifiers.measure_and_check_if_ready_to_act(
                        self.current_time_step,
                        &self.edges,
                        &self.vertices,
                    ) {
                        modifiers.act(&mut self.edges, &mut self.vertices);
                    }
                }
            }
            self.forward_messages_on_edges(ready_to_observe);
        }
    }

    pub fn run_and_save(mut self, hdf5_file: &hdf5_metno::file::File) {
        self.run();
        self.save(hdf5_file);
    }

    fn message_generate(&mut self, ready_to_observe: bool) {
        for (source_id, source) in self.vertices.iter_mut().enumerate() {
            if self.random_engine.sample_will_send_msg() {
                let destination = self.random_engine.sample_destination(source_id);
                if ready_to_observe {
                    source.increment_num_created_messages();
                }
                let msg = {
                    let mut msg = Message::new(source_id, destination, self.current_time_step)
                        .expect("source should be different then destination");
                    msg.step(
                        &self.graph_with_cache,
                        &mut self.random_engine,
                        self.config.routing_method.clone(),
                        self.current_time_step,
                    );
                    msg
                };
                match msg.state() {
                    MessageState::InEdge { edge: (from, to) } => {
                        let edge_id = self.graph_with_cache.get_edge_id(from, to);
                        self.edges[edge_id].add_message(msg, ready_to_observe);
                    }
                    _ => {
                        unreachable!("Message should be in edge after generation.");
                    }
                };
            }
        }
    }
    fn forward_messages_on_edges(&mut self, ready_to_observe: bool) {
        self.random_engine
            .shuffle_slice(&mut self.shuffled_edge_indices);
        let mut transit_queue: Vec<(usize, Message)> = Vec::new();
        for edge_id in self.shuffled_edge_indices.iter() {
            let edge = &mut self.edges[*edge_id];
            for mut msg in edge.messages_to_deliver_mut() {
                msg.step(
                    &self.graph_with_cache,
                    &mut self.random_engine,
                    self.config.routing_method.clone(),
                    self.current_time_step,
                );
                match msg.state() {
                    MessageState::InEdge { edge: (from, to) } => {
                        if ready_to_observe {
                            self.vertices[from].increment_num_traveling_msgs();
                        }
                        let edge_id = self.graph_with_cache.get_edge_id(from, to);
                        transit_queue.push((edge_id, msg));
                    }
                    MessageState::Arrived { .. } => {
                        let ideal_distance = self
                            .graph_with_cache
                            .get_distance(msg.source(), msg.destination());
                        self.vertices[msg.destination()].update_statistics(
                            &msg,
                            ideal_distance,
                            ready_to_observe,
                        );
                    }
                    _ => {
                        unreachable!("Message should be either in edge or arrived after step.");
                    }
                };
            }
        }
        for (edge_id, msg) in transit_queue.into_iter() {
            self.edges[edge_id].add_message(msg, ready_to_observe);
        }
    }

    fn save(mut self, file: &hdf5_metno::file::File) {
        let results_group = file
            .group("simulations_results")
            .or_else(|_| file.create_group("simulations_results"))
            .or_else(|_| file.group("simulations_results"))
            .expect("Could not open or create results group");

        let group = results_group
            .create_group(&self.config.uuid.to_string())
            .expect("Could not create HDF5 group");
        group
            .new_dataset_builder()
            .with_data(&serde_json::to_vec(&self.config).unwrap())
            .create("json_string")
            .expect("Couldn't create HDF5Dataset");
        group
            .new_dataset_builder()
            .with_data(&self.vertices)
            .create("vertices_attributes")
            .expect("Couldn't create HDF5Dataset");
        let edges_hdf5: Vec<HDF5Edge> = self.edges.iter().map(|e| e.to_hdf5()).collect();
        group
            .new_dataset_builder()
            .with_data(&edges_hdf5)
            .create("edges_attributes")
            .expect("Couldn't create HDF5Dataset");
        self.observers.drain(..).for_each(|o| o.save(&group));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs::File;
    use std::io::Write;
    use std::sync::Arc;
    use uuid::Uuid;

    fn write_temp_edgelist(file_stem: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}.edgelist", file_stem, Uuid::new_v4()));
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    fn build_config(
        graph_file_name: &str,
        max_iterations: u64,
        warm_up_iterations: Option<u64>,
    ) -> SimulationConfigurationItem {
        build_config_with_modifiers(
            graph_file_name,
            max_iterations,
            warm_up_iterations,
            json!([]),
        )
    }

    fn build_config_with_modifiers(
        graph_file_name: &str,
        max_iterations: u64,
        warm_up_iterations: Option<u64>,
        modifiers: serde_json::Value,
    ) -> SimulationConfigurationItem {
        let cfg = json!({
            "uuid": Uuid::new_v4().to_string(),
            "graph_file_name": graph_file_name,
            "message_generation": 1.0,
            "max_iterations": max_iterations,
            "warm_up_iterations": warm_up_iterations,
            "random_seed": 42,
            "routing_method": "minimal_paths",
            "observers": [],
            "modifiers": modifiers
        });
        serde_json::from_value(cfg).unwrap()
    }

    #[test]
    fn simulation_initializes_internal_state() {
        let graph_cache = crate::test_utils::dummy_graph_cache();
        let config = build_config("simulation_new_initializes_internal_state", 5, Some(0));

        let simulation = Simulation::new(graph_cache.clone(), &config);
        assert_eq!(simulation.current_time_step, 0);
        assert_eq!(simulation.vertices.len(), graph_cache.node_count());
        assert_eq!(simulation.edges.len(), graph_cache.edge_count());
        assert_eq!(
            simulation.shuffled_edge_indices,
            (0..graph_cache.edge_count()).collect::<Vec<_>>()
        );
        assert!(simulation.observers.is_empty());
        assert!(simulation.modifiers.is_empty());
    }

    #[test]
    fn run_and_save_creates_simulation_group_and_datasets() {
        let edge_list = "3\n2\n0 1\n1 2\n";
        let graph_path = write_temp_edgelist(
            "run_and_save_creates_simulation_group_and_datasets",
            edge_list,
        );
        let graph_cache = Arc::new(PreComputedGraph::from_edgelist_file(&graph_path));
        let config = build_config(graph_path.to_str().unwrap(), 3, Some(0));
        let simulation_uuid = config.uuid.to_string();

        let hdf5_path =
            std::env::temp_dir().join(format!("simulation_test_{}.hdf5", Uuid::new_v4()));
        let hdf5_file = hdf5_metno::file::File::create(&hdf5_path).unwrap();

        let simulation = Simulation::new(graph_cache.clone(), &config);
        simulation.run_and_save(&hdf5_file);

        let results_group = hdf5_file.group("simulations_results").unwrap();
        let simulation_group = results_group.group(&simulation_uuid).unwrap();

        let vertices_ds = simulation_group.dataset("vertices_attributes").unwrap();
        let edges_ds = simulation_group.dataset("edges_attributes").unwrap();

        let vertices_shape = vertices_ds.shape();
        let edges_shape = edges_ds.shape();

        assert_eq!(vertices_shape[0], graph_cache.node_count());
        assert_eq!(edges_shape[0], graph_cache.edge_count());

        std::fs::remove_file(graph_path).unwrap();
        std::fs::remove_file(hdf5_path).unwrap();
    }

    #[test]
    fn run_with_modifier_exercises_modifier_lifecycle_and_persists_results() {
        let edge_list = "3\n2\n0 1\n1 2\n";
        let graph_path = write_temp_edgelist(
            "run_with_modifier_exercises_modifier_lifecycle_and_persists_results",
            edge_list,
        );
        let graph_cache = Arc::new(PreComputedGraph::from_edgelist_file(&graph_path));
        let config = build_config_with_modifiers(
            graph_path.to_str().unwrap(),
            4,
            Some(0),
            json!([{
                "type": "ModifierEdgeCapacity",
                "free_flow_rate": 0.5,
                "free_flow_sampling_time": 1,
                "minimal_capacity": 1,
                "multiplier": 1.0
            }]),
        );
        let simulation_uuid = config.uuid.to_string();

        let hdf5_path =
            std::env::temp_dir().join(format!("simulation_modifier_test_{}.hdf5", Uuid::new_v4()));
        let hdf5_file = hdf5_metno::file::File::create(&hdf5_path).unwrap();

        let simulation = Simulation::new(graph_cache.clone(), &config);
        assert_eq!(simulation.modifiers.len(), 1);
        simulation.run_and_save(&hdf5_file);

        let results_group = hdf5_file.group("simulations_results").unwrap();
        let simulation_group = results_group.group(&simulation_uuid).unwrap();
        let edges_ds = simulation_group.dataset("edges_attributes").unwrap();
        let edges_data: Vec<HDF5Edge> = edges_ds.read_raw().unwrap();

        assert_eq!(edges_data.len(), graph_cache.edge_count());

        std::fs::remove_file(graph_path).unwrap();
        std::fs::remove_file(hdf5_path).unwrap();
    }
}

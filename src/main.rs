mod cli;
mod graph_dynamics;
mod graph_structure;
mod modifiers;
mod observers;
mod random_engine;
mod schema;
mod simulation;
#[cfg(test)]
mod test_utils;
use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::time::{Duration, Instant};
use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    graph_structure::PreComputedGraph, schema::SimulationConfigurationItem, simulation::Simulation,
};

type GraphCacheCell = OnceLock<Arc<PreComputedGraph>>;
type GraphCacheHandle = Arc<GraphCacheCell>;
type GraphCacheRegistry = RwLock<HashMap<String, GraphCacheHandle>>;

fn main() {
    let cli_args = cli::Cli::parse();
    if let Err(err) = run(cli_args) {
        panic!("{err}");
    }
}

fn run(cli_args: cli::Cli) -> Result<(), String> {
    println!("    --- Simulation Config --");
    println!("    ├─ Json file: {:?}", cli_args.json_path);
    println!("    ├─ Threads: {}", cli_args.threads);

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli_args.threads)
        .build_global()
        .map_err(|e| format!("Failed to build thread pool: {e}"))?;

    let json_file = std::fs::File::open(&cli_args.json_path)
        .map_err(|_| format!("Couldn't open json file {:?}", cli_args.json_path))?;

    let json: serde_json::Value =
        serde_json::from_reader(json_file).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    if !schema::validate_json_simulation(&json, &crate::schema::JSON_SIMULATION_VALIDATOR) {
        return Err("Json in the wrong schema, check the schema.json file".to_string());
    }

    let configs: schema::SimulationConfiguration = serde_json::from_value(json)
        .map_err(|e| format!("Failed to deserialize JSON into SimulationConfiguration: {e}"))?;

    let hdf5_file = init_hdf5_file(&cli_args)?;

    let configs_grouped_by_graph = group_by_graph(&configs);
    println!(
        "    ├─ Isomorphic Groups: {}",
        configs_grouped_by_graph.len()
    );
    println!("    └─ Total Simulations: {}", configs.len());

    let number_of_remaining_simulations_per_cache: HashMap<String, AtomicUsize> =
        configs_grouped_by_graph
            .iter()
            .map(|(key, simulations)| (key.clone(), AtomicUsize::from(simulations.len())))
            .collect();

    let filename_to_graphcache = build_graph_cache_registry(&configs_grouped_by_graph);
    let flatten_configs: Vec<&SimulationConfigurationItem> =
        configs_grouped_by_graph.values().flatten().collect();

    let num_caches_in_memory = AtomicUsize::new(0);
    let tracker_pretty_print = SimulationTracker::new(flatten_configs.len());
    let graphs_datagroup = hdf5_file
        .group("graphs")
        .map_err(|e| format!("Failed to open graphs group: {e}"))?;

    flatten_configs.par_iter().for_each(|config| {
        let graph_lock =
            { filename_to_graphcache.read().unwrap()[&config.graph_file_name].clone() };
        let graph_file_name = &config.graph_file_name;

        let graph_cache = graph_lock.get_or_init(|| {
            let graph_cache = Arc::new(PreComputedGraph::from_edgelist_file(std::path::Path::new(
                graph_file_name,
            )));
            let graph_uuid = std::path::Path::new(graph_file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap();

            let graph_group = graphs_datagroup.create_group(graph_uuid).unwrap();
            num_caches_in_memory.fetch_add(1, Ordering::SeqCst);
            graph_cache.save_edgelist_hdf5(&graph_group);
            graph_cache
        });

        let simulation = Simulation::new(graph_cache.clone(), config);
        let start = Instant::now();
        simulation.run_and_save(&hdf5_file);
        let duration = start.elapsed();

        let remaining = number_of_remaining_simulations_per_cache[graph_file_name]
            .fetch_sub(1, Ordering::SeqCst);
        if remaining == 1 {
            num_caches_in_memory.fetch_sub(1, Ordering::SeqCst);
            filename_to_graphcache
                .write()
                .unwrap()
                .remove(graph_file_name);
        }

        tracker_pretty_print.record_and_print(
            &config.graph_file_name,
            num_caches_in_memory.load(Ordering::SeqCst),
            duration,
        );
    });

    Ok(())
}

fn init_hdf5_file(cli_args: &cli::Cli) -> Result<hdf5_metno::file::File, String> {
    let hdf5_file_name = cli_args
        .output_file_hdf5
        .clone()
        .unwrap_or(cli_args.json_path.with_extension("hdf5"));

    if hdf5_file_name.exists() {
        if cli_args.force {
            std::fs::remove_file(&hdf5_file_name)
                .map_err(|_| format!("Could not remove existing HDF5 file {:?}", hdf5_file_name))?;
        } else {
            return Err(format!(
                "HDF5 file {:?} already exists, use --force to overwrite",
                hdf5_file_name
            ));
        }
    }
    if let Some(parent) = hdf5_file_name.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            format!(
                "Could not create parent directories for HDF5 file {:?}",
                hdf5_file_name
            )
        })?;
    }
    let hdf5_file = hdf5_metno::file::File::create(&hdf5_file_name)
        .map_err(|_| format!("Could not create HDF5 file {:?}", hdf5_file_name))?;
    hdf5_file
        .create_group("graphs")
        .map_err(|e| format!("Could not create graphs group: {e}"))?;

    Ok(hdf5_file)
}

fn build_graph_cache_registry(
    grouped: &HashMap<String, Vec<SimulationConfigurationItem>>,
) -> GraphCacheRegistry {
    let filename_to_graphcache: GraphCacheRegistry = RwLock::new(HashMap::new());

    for filename in grouped.keys() {
        filename_to_graphcache
            .write()
            .unwrap()
            .entry(filename.to_string())
            .or_insert(Arc::new(OnceLock::new()));
    }

    filename_to_graphcache
}

pub fn group_by_graph(
    simulation_configs: &schema::SimulationConfiguration,
) -> HashMap<String, Vec<SimulationConfigurationItem>> {
    let mut result = HashMap::new();
    for config in simulation_configs.iter() {
        result
            .entry(config.graph_file_name.clone())
            .or_insert_with(Vec::new)
            .push(config.clone());
    }
    result
}

struct SimulationTracker {
    total: usize,
    finished: AtomicUsize,
    start_time: Instant,
}

impl SimulationTracker {
    fn new(total: usize) -> Self {
        Self {
            total,
            finished: AtomicUsize::new(0),
            start_time: Instant::now(),
        }
    }

    fn record_and_print(&self, graph_name: &str, caches: usize, duration: Duration) {
        let finished = self.finished.fetch_add(1, Ordering::SeqCst) + 1;
        let elapsed = self.start_time.elapsed();

        let avg = elapsed.div_f64(finished as f64);
        let remaining = if self.total > finished {
            self.total - finished
        } else {
            0
        };
        let eta = avg.mul_f64(remaining as f64);

        println!(
            "[{}/{}] Graph: {} | Caches: {} | Time: {:.2?} | Avg: {:.2?} | ETA: {:.2?}",
            finished, self.total, graph_name, caches, duration, avg, eta
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Map;

    use super::*;
    use crate::schema::{RoutingMethod, SimulationConfiguration, SimulationConfigurationItem};
    use schema::ModifierEdgeCapacity;
    use std::num::NonZero;
    use std::path::PathBuf;

    fn cfg(graph: &str) -> SimulationConfigurationItem {
        SimulationConfigurationItem {
            graph_file_name: graph.to_string(),
            max_iterations: NonZero::new(10).unwrap(),
            uuid: uuid::Uuid::new_v4(),
            graph_generation_info: Map::new(),
            random_seed: Some(123),
            message_generation: 0.1,
            routing_method: RoutingMethod::RandomWalk,
            warm_up_iterations: Some(0),
            modifiers: vec![ModifierEdgeCapacity {
                free_flow_rate: 0.8,
                free_flow_sampling_time: 10,
                multiplier: 1.0,
                minimal_capacity: 1,
                type_: serde_json::json! {"edge_capacity"},
            }],
            observers: Vec::new(),
        }
    }

    #[test]
    fn group_by_graph_groups_items_by_graph_file_name() {
        let input = SimulationConfiguration(vec![
            cfg("a.edgelist"),
            cfg("b.edgelist"),
            cfg("a.edgelist"),
        ]);

        let grouped = group_by_graph(&input);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["a.edgelist"].len(), 2);
        assert_eq!(grouped["b.edgelist"].len(), 1);
    }

    #[test]
    fn group_by_graph_empty_input_returns_empty_map() {
        let input = SimulationConfiguration(Vec::new());
        let grouped = group_by_graph(&input);
        assert!(grouped.is_empty());
    }

    #[test]
    fn build_graph_cache_registry_creates_an_entry_per_graph_name() {
        let grouped = HashMap::from([
            ("g1.edgelist".to_string(), vec![cfg("g1.edgelist")]),
            ("g2.edgelist".to_string(), vec![cfg("g2.edgelist")]),
        ]);

        let registry = build_graph_cache_registry(&grouped);
        let guard = registry.read().unwrap();

        assert_eq!(guard.len(), 2);
        assert!(guard.contains_key("g1.edgelist"));
        assert!(guard.contains_key("g2.edgelist"));
    }

    #[test]
    fn init_hdf5_file_refuses_existing_file_when_force_is_false() {
        let dir = std::env::temp_dir();
        let unique = format!(
            "graph_traffic_sim_test_{}_{}.hdf5",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        );
        let hdf5_path = dir.join(unique);

        std::fs::write(&hdf5_path, b"placeholder").unwrap();

        let cli_args = crate::cli::Cli {
            json_path: PathBuf::from("dummy.json"),
            output_file_hdf5: Some(hdf5_path.clone()),
            threads: 1,
            force: false,
        };

        let result = init_hdf5_file(&cli_args);
        assert!(result.is_err());

        let _ = std::fs::remove_file(hdf5_path);
    }

    #[test]
    fn simulation_tracker_record_increments_finished_counter() {
        let tracker = SimulationTracker::new(3);
        tracker.record_and_print("graph_a", 1, Duration::from_millis(2));
        tracker.record_and_print("graph_a", 1, Duration::from_millis(3));

        let finished = tracker.finished.load(Ordering::SeqCst);
        assert_eq!(finished, 2);
    }
}

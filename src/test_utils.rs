#![cfg(test)]

use crate::PreComputedGraph;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

pub fn write_temp_edgelist(file_stem: &str, content: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("{}_{}.edgelist", file_stem, "eafas"));
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}
pub fn dummy_graph_cache() -> Arc<PreComputedGraph> {
    let edge_list = "3\n2\n0 1\n1 2\n";
    let graph_path = write_temp_edgelist("simulation_new_initializes_internal_state", edge_list);
    Arc::new(PreComputedGraph::from_edgelist_file(&graph_path))
}
pub fn dummy_random_generator() -> crate::random_engine::RandomEngine {
    crate::random_engine::RandomEngine::new(None, 0.5, 1).unwrap()
}

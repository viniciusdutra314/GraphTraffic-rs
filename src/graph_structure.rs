use num_bigint::BigUint;
use num_rational::Ratio;
use num_traits::ToPrimitive;
use petgraph::Graph;
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences};
use petgraph::{self, Directed, Undirected, csr::Csr};
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::num_traits::{One, Zero};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Deref;

use petgraph::visit::{EdgeCount, IntoNeighbors, NodeCount, NodeIndexable};
use std::path::Path;

type AdjacencyList<N = (), E = ()> = Csr<N, E, Undirected, usize>;
type ShortestPathDAG = DAG<Csr<u16, f32, Directed, usize>>;

struct DAG<G> {
    inner: G,
}

impl<G> Deref for DAG<G> {
    type Target = G;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<G> DAG<G>
where
    G: NodeCount + EdgeCount + NodeIndexable,
    for<'a> &'a G: IntoEdgeReferences<NodeId = G::NodeId>,
{
    pub fn new(graph: G) -> Self {
        use petgraph::graph::NodeIndex;
        let mut graph_to_toposort =
            Graph::<(), (), Directed>::with_capacity(graph.node_count(), graph.edge_count());
        for _ in 0..graph.node_count() {
            graph_to_toposort.add_node(());
        }
        for edge in graph.edge_references() {
            let source = NodeIndex::new(graph.to_index(edge.source()));
            let target = NodeIndex::new(graph.to_index(edge.target()));
            graph_to_toposort.add_edge(source, target, ());
        }
        if petgraph::algo::is_cyclic_directed(&graph_to_toposort) {
            panic!("Directed graph is cyclic");
        }
        Self { inner: graph }
    }
}

pub struct PreComputedGraph {
    adjacency_list: AdjacencyList,
    edge_lookup: HashMap<(usize, usize), usize>,
    shortest_path_dags: Vec<ShortestPathDAG>,
}

fn get_sorted_edges_from_edgefile(path: &Path) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let file =
        File::open(path).unwrap_or_else(|e| panic!("Cannot open edgelist {}: {e}", path.display()));
    let mut reader = BufReader::new(file);
    let mut v_str = String::new();
    reader
        .read_line(&mut v_str)
        .expect("Cannot read edgelist header");
    let v: usize = v_str
        .trim()
        .parse()
        .expect("Invalid vertex count on edgelist line 1");
    let mut e_str = String::new();
    reader
        .read_line(&mut e_str)
        .expect("Cannot read edgelist header");
    let e: usize = e_str
        .trim()
        .parse()
        .expect("Invalid edge count on edgelist line 2");
    assert!(
        v >= 2 && e > 0,
        "Edgelist must contain at least two vertices and one edge"
    );
    let mut sorted_edges = Vec::with_capacity(2 * e);
    let mut file_edges = Vec::with_capacity(e);
    let mut seen = HashSet::with_capacity(e);
    for (index, line) in reader.lines().enumerate() {
        let number = index + 3;
        let line = line.unwrap_or_else(|err| panic!("Cannot read edgelist line {number}: {err}"));
        let mut parts = line.split_whitespace();
        let from: usize = parts
            .next()
            .unwrap_or_else(|| panic!("Malformed edgelist line {number}"))
            .parse()
            .unwrap_or_else(|_| panic!("Malformed edgelist line {number}"));
        let to: usize = parts
            .next()
            .unwrap_or_else(|| panic!("Malformed edgelist line {number}"))
            .parse()
            .unwrap_or_else(|_| panic!("Malformed edgelist line {number}"));
        assert!(parts.next().is_none(), "Malformed edgelist line {number}");
        assert!(
            from < v && to < v,
            "Vertex out of range on edgelist line {number}"
        );
        assert_ne!(from, to, "Self-loop on edgelist line {number}");
        let pair = (from.min(to), from.max(to));
        assert!(
            seen.insert(pair),
            "Duplicate edge on edgelist line {number}"
        );
        file_edges.push(pair);
        sorted_edges.push((from, to));
        sorted_edges.push((to, from));
    }
    assert_eq!(
        file_edges.len(),
        e,
        "Edgelist declares {e} edges but contains {}",
        file_edges.len()
    );
    sorted_edges.sort_unstable();
    (sorted_edges, file_edges)
}

impl PreComputedGraph {
    pub fn from_edgelist_file(path: &Path) -> Self {
        let (sorted_edges_duplicated, file_edges) = get_sorted_edges_from_edgefile(path);
        let adjacency_list = AdjacencyList::from_sorted_edges(&sorted_edges_duplicated)
            .unwrap_or_else(|e| panic!("Invalid edgelist {}: {e:?}", path.display()));
        // These IDs index Simulation.edges, whose capacities follow the edgelist line order.
        let edge_lookup = file_edges
            .into_iter()
            .enumerate()
            .map(|(id, edge)| (edge, id))
            .collect();
        Self::with_edge_lookup(adjacency_list, edge_lookup)
    }
    pub fn from_adjlist(adjacency_list: AdjacencyList) -> Self {
        let edge_lookup = {
            let mut edge_lookup = HashMap::new();
            let mut edge_id = 0;
            for edge in adjacency_list.edge_references() {
                let (u, v) = (edge.source(), edge.target());
                let pair = if u < v { (u, v) } else { (v, u) };
                if !edge_lookup.contains_key(&pair) {
                    edge_lookup.insert(pair, edge_id);
                    edge_id += 1;
                }
            }
            edge_lookup
        };
        Self::with_edge_lookup(adjacency_list, edge_lookup)
    }

    fn with_edge_lookup(
        adjacency_list: AdjacencyList,
        edge_lookup: HashMap<(usize, usize), usize>,
    ) -> Self {
        let shortest_path_dags: Vec<ShortestPathDAG> = (0..adjacency_list.node_count())
            .into_par_iter()
            .map(|source| build_shortest_path_dag(&adjacency_list, source))
            .collect();
        Self {
            adjacency_list,
            edge_lookup,
            shortest_path_dags,
        }
    }

    pub fn node_count(&self) -> usize {
        self.adjacency_list.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_lookup.len()
    }

    pub fn save_edgelist_hdf5(&self, group: &hdf5_metno::Group) {
        #[derive(hdf5_metno::H5Type)]
        #[repr(C)]
        struct EdgeWithID {
            source: usize,
            target: usize,
            edge_id: usize,
        }

        let hdf5_edges: Vec<EdgeWithID> = self
            .edge_lookup
            .iter()
            .map(|(k, v)| EdgeWithID {
                source: k.0,
                target: k.1,
                edge_id: *v,
            })
            .collect();
        group
            .new_dataset_builder()
            .with_data(&hdf5_edges)
            .create("edgelist")
            .unwrap();
    }

    pub fn neighbors_slice(&self, source: usize) -> &[usize] {
        self.adjacency_list.neighbors_slice(source)
    }

    pub fn get_distance(&self, source: usize, target: usize) -> u16 {
        self.shortest_path_dags[target][source]
    }
    pub fn get_edge_id(&self, source: usize, target: usize) -> usize {
        if source > target {
            self.edge_lookup.get(&(target, source)).copied().unwrap()
        } else {
            self.edge_lookup.get(&(source, target)).copied().unwrap()
        }
    }

    pub fn sample_closer_neighbor(
        &self,
        source: usize,
        destination: usize,
        rng: &mut impl Rng,
    ) -> usize {
        use rand::distr::weighted::WeightedIndex;
        let spt = &self.shortest_path_dags[destination];
        let neighbors = spt.neighbors_slice(source);
        let weights = spt.edges_slice(source);
        let dist = WeightedIndex::new(weights).expect("Failed to create weighted index");
        neighbors[dist.sample(rng)]
    }
}

fn build_shortest_path_dag<N, E>(graph: &AdjacencyList<N, E>, source: usize) -> ShortestPathDAG {
    let (distances, sigmas, mut dag_edges) = distance_and_sigma_vec(&graph, source);
    if distances.iter().any(|d| *d == u16::MAX) {
        panic!("BFS failed to reach all nodes, graph is not connected");
    }
    for dag_edge in dag_edges.iter_mut() {
        let prop = Ratio::new(sigmas[dag_edge.1].clone(), sigmas[dag_edge.0].clone());
        dag_edge.2 = prop.to_f32().unwrap();
    }
    // sorts based on (usize,usize) and simply ignore f32
    dag_edges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let sp = {
        let mut sp = Csr::<u16, f32, Directed, usize>::from_sorted_edges(&dag_edges).unwrap();
        // Assign vertex distances
        let indices: Vec<usize> = sp.node_references().map(|(id, _)| id).collect();
        for index in indices {
            sp[index] = distances[index];
        }
        sp
    };
    DAG::new(sp)
}

fn distance_and_sigma_vec<N, E>(
    graph: &AdjacencyList<N, E>,
    source: usize,
) -> (Vec<u16>, Vec<BigUint>, Vec<(usize, usize, f32)>) {
    let num_nodes = graph.node_count();
    let mut sigmas = vec![BigUint::zero(); num_nodes];
    let mut distances = vec![u16::MAX; num_nodes];
    let mut dag_edges: Vec<(usize, usize, f32)> = Vec::new();
    let mut queue = VecDeque::new();
    sigmas[source] = BigUint::one();
    distances[source] = 0;
    queue.push_back(source);
    // BFS: Calculate Distances, Path Counts (Sigma), and DAG Edges
    while let Some(v) = queue.pop_front() {
        let dist_v = distances[v];
        //fighting the borrow checker moment
        let current_sigma = std::mem::take(&mut sigmas[v]);

        for nb in graph.neighbors(v) {
            if distances[nb] == u16::MAX {
                distances[nb] = dist_v + 1;
                sigmas[nb] = current_sigma.clone();
                queue.push_back(nb);
                dag_edges.push((nb, v, 0.0));
            } else if distances[nb] == dist_v + 1 {
                sigmas[nb] += &current_sigma;
                dag_edges.push((nb, v, 0.0));
            }
        }
        sigmas[v] = current_sigma;
    }
    (distances, sigmas, dag_edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array2, ArrayView2};
    use petgraph;
    use rustworkx_core::generators::{barabasi_albert_graph, grid_graph, path_graph, star_graph};
    use std::io::Write;
    type UnGraph = petgraph::graph::Graph<(), (), Undirected, usize>;

    fn from_un_graph_to_adjlist(graph: UnGraph) -> AdjacencyList {
        let edges = {
            let mut edges = Vec::new();
            for edge in graph.raw_edges() {
                edges.push((edge.source().index(), edge.target().index()));
                edges.push((edge.target().index(), edge.source().index()));
            }
            edges.sort_unstable();
            edges
        };
        AdjacencyList::from_sorted_edges(&edges).unwrap()
    }

    fn have_distance_matrix_proprieties(d_matrix: &ArrayView2<u16>) -> bool {
        if !d_matrix.diag().iter().all(|&x| x == 0) {
            return false;
        }
        if d_matrix != &d_matrix.t() {
            return false;
        }
        let n = d_matrix.nrows();
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    if d_matrix[(i, j)] > d_matrix[(i, k)] + d_matrix[(k, j)] {
                        return false;
                    }
                }
            }
        }
        true
    }
    #[test]
    fn test_compare_distances_with_petgraph_algorithms() {
        let mut test_cases: Vec<UnGraph> = vec![
            grid_graph(Some(10), Some(10), None, || (), || (), false).unwrap(),
            path_graph(Some(10), None, || (), || (), false).unwrap(),
            star_graph(Some(100), None, || (), || (), false, false).unwrap(),
            barabasi_albert_graph(50, 3, None, None, || (), || ()).unwrap(),
        ];

        for graph in test_cases.drain(..).map(|g| from_un_graph_to_adjlist(g)) {
            let cache = PreComputedGraph::from_adjlist(graph.clone());
            let n = cache.node_count();
            let d_matrix = Array2::from_shape_fn((n, n), |(i, j)| cache.get_distance(i, j));
            let d_matrix_view = d_matrix.view();
            assert!(have_distance_matrix_proprieties(&d_matrix_view));

            let expected_dists = petgraph::algo::floyd_warshall(&graph, |_| 1u16).unwrap();

            for i in 0..graph.node_count() {
                for j in 0..graph.node_count() {
                    let expected_dist = *expected_dists.get(&(i, j)).unwrap();
                    assert_eq!(
                        d_matrix[(i, j)],
                        expected_dist,
                        "Distance mismatch between node {} and {}",
                        i,
                        j
                    );
                }
            }
        }
    }
    #[test]
    fn test_sigma_calculation_grid_graph() {
        let rows = 10;
        let cols = 10;
        let g: UnGraph = grid_graph(Some(rows), Some(cols), None, || (), || (), false).unwrap();
        let graph = from_un_graph_to_adjlist(g);
        let (_, sigmas, _) = distance_and_sigma_vec(&graph, 0);
        for r in 0..rows {
            for c in 0..cols {
                let node_idx = r * cols + c;
                let n = r + c;
                let k = r;
                let mut expected_sigma = BigUint::one();
                for i in 0..k {
                    expected_sigma = (expected_sigma * (n - i)) / (i + 1);
                }

                assert_eq!(sigmas[node_idx], expected_sigma);
            }
        }
    }
    #[test]
    fn test_new_from_edgelist_with_temp_file() {
        let temp_path = std::env::temp_dir().join("test_graph_read.edgelist");
        {
            let mut file = File::create(&temp_path).unwrap();
            writeln!(file, "3").unwrap();
            writeln!(file, "2").unwrap();
            writeln!(file, "0 1").unwrap();
            writeln!(file, "1 2").unwrap();
        }

        let graph = PreComputedGraph::from_edgelist_file(&temp_path);
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.get_edge_id(0, 1), 0);
        assert_eq!(graph.get_edge_id(1, 2), 1);

        std::fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn edgelist_rejects_invalid_edges_and_counts() {
        use uuid::Uuid;
        for (contents, expected) in [
            ("3\n1\n0 0\n", "Self-loop"),
            ("3\n2\n0 1\n0 1\n", "Duplicate edge"),
            ("3\n2\n0 1\n1 0\n", "Duplicate edge"),
            ("3\n1\n0\n", "Malformed"),
            ("3\n1\n0 1 2\n", "Malformed"),
            ("3\n2\n0 1\n", "declares 2 edges"),
            ("3\n1\n0 1\n1 2\n", "declares 1 edges"),
            ("3\n1\n0 3\n", "out of range"),
            ("4\n2\n0 1\n2 3\n", "connected"),
            ("4\n2\n0 1\n1 2\n", "connected"),
        ] {
            let path = std::env::temp_dir().join(format!("invalid_{}.edgelist", Uuid::new_v4()));
            std::fs::write(&path, contents).unwrap();
            let panic = std::panic::catch_unwind(|| PreComputedGraph::from_edgelist_file(&path))
                .err()
                .expect("invalid edgelist should panic");
            assert!(crate::test_utils::panic_message(panic).contains(expected));
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn test_dag_specific_example() {
        use petgraph::dot::*;
        let mut edges = vec![
            ('a', 'b'),
            ('a', 'c'),
            ('a', 'd'),
            ('b', 'c'),
            ('b', 'e'),
            ('c', 'e'),
            ('d', 'e'),
            ('d', 'f'),
            ('e', 'g'),
            ('f', 'g'),
            ('e', 'h'),
            ('g', 'i'),
            ('h', 'i'),
            ('f', 'h'),
        ]
        .iter()
        .map(|(u, v)| {
            (
                (*u as usize) - ('a' as usize),
                (*v as usize) - ('a' as usize),
            )
        })
        .collect::<Vec<_>>();
        edges.sort_unstable();
        let opposite_direction = edges.iter().map(|(u, v)| (*v, *u)).collect::<Vec<_>>();
        let mut edges_doubled = edges.clone();
        edges_doubled.extend(&opposite_direction);
        edges_doubled.sort_unstable();
        use palette::{LinSrgb, Mix, Srgb};

        fn get_edge_color(prob: f32) -> String {
            let color_low = LinSrgb::new(0.8, 0.8, 0.8);
            let color_high = LinSrgb::new(0.0, 0.3, 0.8);
            let mixed = color_low.mix(color_high, prob);
            let final_color: Srgb<u8> = Srgb::from_linear(mixed);
            format!(
                "\"#{:02x}{:02x}{:02x}\"",
                final_color.red, final_color.green, final_color.blue
            )
        }
        let edges_dummy_attrs = edges
            .iter()
            .map(|(u, v)| (*u, *v, 0))
            .collect::<Vec<(usize, usize, usize)>>();
        let csr_graph_doubled = Csr::from_sorted_edges(&edges_doubled).unwrap();
        let csr_graph =
            Csr::<usize, usize, Undirected, usize>::from_sorted_edges(&edges_dummy_attrs).unwrap();
        let dot_config = vec![
            Config::EdgeNoLabel,
            Config::NodeNoLabel,
            Config::RankDir(RankDir::BT),
        ];
        let graph = PreComputedGraph::from_adjlist(csr_graph_doubled);

        println!("{}", Dot::with_config(&(csr_graph), &dot_config));
        println!("dag dot");
        println!(
            "{}",
            Dot::with_attr_getters(
                &(*graph.shortest_path_dags[0]),
                &dot_config,
                &|_, edge_ref| {
                    let w = *edge_ref.weight();
                    format!(r#"label = "{:.2}", color = {}"#, w, get_edge_color(w))
                },
                &|_, (_, d)| format!(r#"label = {}"#, d),
            )
        );
    }
}

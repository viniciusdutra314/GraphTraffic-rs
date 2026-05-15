# GraphTraffic-rs

Rust simulator for network traffic on connected undirected graphs. It loads graphs from an edge list, runs message-passing simulations with configurable routing, applies optional modifiers/observers, and persists results to a HDF5 file.

It's based on the Traffic Routing Model defined on Chen, Shengyong, Wei Huang, Carlo Cattani, and Giuseppe Altieri. “Traffic Dynamics on Complex Networks: A Survey.” Mathematical Problems in Engineering 2012, no. 1 (2012): 732698. https://doi.org/10.1155/2012/732698. One important diference is that the edges and not the vertices, are thought to transport the messages and have a capacity.


## Highlights

- Multi-threaded simulations via `rayon`.
- JSON configuration validated against `schema.json`.
- Built-in routing strategies and pluggable observers/modifiers.
- HDF5 output with graph metadata and per-simulation results.

## Requirements

- Rust 1.88+
## Build

```bash
cargo build --release
```

## Run

```bash
cargo run -- <path/to/config.json> [--output-file-hdf5 <path/to/output.hdf5>] [--threads <N>] [--force]
```

- `--output-file-hdf5`: optional output file. Defaults to the JSON path with the `.hdf5` extension.
- `--threads`: number of threads (default: ~50% of logical cores).
- `--force`: overwrite the output file if it exists.

## Input graph format (`.edgelist`)

The graph is read from a text file with the following layout:

```
<number_of_vertices>
<number_of_edges>
<source_0> <target_0>
<source_1> <target_1>
...
```

- Nodes are identified by integer indices (0-based and compact interval [0,N-1] is expected ).
- The graph is treated as **undirected**. Each listed edge is duplicated internally.
- Self-loops are rejected.
- Parallel edges are deduplicated.
- The graph must be connected; otherwise, shortest-path precomputation will panic.

## Configuration (`config.json`)

The config file is a JSON **array** of simulation items. The schema is defined in `schema.json`.

### Example

```json
[
  {
    "uuid": "123e4567-e89b-12d3-a456-426614174000",
    "graph_file_name": "graphs/grid.edgelist",
    "message_generation": 0.2,
    "max_iterations": 1000,
    "warm_up_iterations": 100,
    "random_seed": 42,
    "routing_method": "minimal_paths",
    "graph_generation_info": { "generator": "barabasi", "n":1000, "m":3 },
    "modifiers": [
      {
        "type": "ModifierEdgeCapacity",
        "free_flow_rate": 0.8,
        "free_flow_sampling_time": 10
      }
    ],
    "observers": [
      { "type": "ObserverEdgeQueue" },
      { "type": "ObserverEdgeCapacity", "update_interval": 10 },
      { "type": "ObserverTotalMessages" }
    ]
  }
]
```

### Fields

- `uuid` (string, required): Unique simulation ID.
- `graph_file_name` (string, required): Path to a `.edgelist` file.
- `message_generation` (number, required): Probability in `(0, 1]` of generating a message at each vertex per time step.
- `max_iterations` (integer, required): Total simulation steps (exclusive minimum > 0).
- `warm_up_iterations` (integer, optional): Steps to skip before observers/modifiers activate.
- `random_seed` (integer, optional): Deterministic RNG seed.
- `routing_method` (required): One of
  - `"minimal_paths"`
  - `"random_walk"`
  - `{ "limited_visibility": <non-negative integer> }`
- `graph_generation_info` (object, optional): Opaque metadata passed through into the output.
- `modifiers` (array, optional):
  - `ModifierEdgeCapacity` with fields `free_flow_rate` (0,1] and `free_flow_sampling_time` (>= 0).
- `observers` (array, optional):
  - `ObserverEdgeQueue`
  - `ObserverEdgeReceivedMessages`
  - `ObserverEdgeCapacity` (requires `update_interval` >= 1)
  - `ObserverTotalMessages`

## Output HDF5 layout

The simulator writes a single HDF5 file with (at least) the following layout:

- `/graphs/<graph_uuid>/edgelist`: edge list with assigned edge IDs.
- `/simulations_results/<uuid>/json_string`: serialized config.
- `/simulations_results/<uuid>/vertices_attributes`: per-vertex counters (arrivals, travel time, etc.).
- `/simulations_results/<uuid>/edges_attributes`: per-edge capacity and total processed message counts.
- Observer datasets/groups under `/simulations_results/<uuid>/` (one per enabled observer).

## License

MIT (see `LICENSE`).

include!(concat!(env!("OUT_DIR"), "/generated_types.rs"));

use crate::graph_dynamics::Edge;
use std::fs::File;
use std::io::{BufRead, BufReader};

impl SimulationConfigurationItem {
    pub fn create_initial_edges(&self, edge_count: usize) -> Vec<Edge> {
        let path = &self.initial_capacity;
        let file = File::open(path)
            .unwrap_or_else(|err| panic!("Cannot open initial_capacity {path}: {err}"));
        let mut edges = Vec::with_capacity(edge_count);
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line.unwrap_or_else(|err| {
                panic!("Cannot read initial_capacity line {}: {err}", index + 1)
            });
            let capacity: usize = line.trim().parse().unwrap_or_else(|_| {
                panic!(
                    "Invalid positive integer on initial_capacity line {}",
                    index + 1
                )
            });
            assert!(
                capacity > 0,
                "Zero capacity on initial_capacity line {}",
                index + 1
            );
            assert!(
                edges.len() < edge_count,
                "initial_capacity has more than {edge_count} capacities"
            );
            edges.push(Edge::new(capacity).unwrap());
        }
        assert_eq!(
            edges.len(),
            edge_count,
            "initial_capacity has {} capacities; expected {edge_count}",
            edges.len()
        );
        edges
    }
}

pub type RoutingMethod = SimulationConfigurationItemRoutingMethod;
pub static JSON_SIMULATION_VALIDATOR: std::sync::LazyLock<jsonschema::Validator> =
    std::sync::LazyLock::new(|| {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../schema.json")).unwrap();
        jsonschema::options().build(&schema).unwrap()
    });

pub fn validate_json_simulation(
    json: &serde_json::Value,
    validator: &jsonschema::Validator,
) -> bool {
    if !validator.is_valid(json) {
        println!("Invalid JSON:");
        for error in validator.iter_errors(json) {
            println!("{}", error);
            println!("{}", error.schema_path());
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_config_item() -> serde_json::Value {
        json!({
            "uuid": "123e4567-e89b-12d3-a456-426614174000",
            "graph_file_name": "graph.txt",
            "initial_capacity": "capacity.txt",
            "message_generation": 0.5,
            "max_iterations": 100,
            "warm_up_iterations": 10,
            "random_seed": 42,
            "routing_method": "minimal_paths",
            "observers": []
        })
    }

    #[test]
    fn test_valid_json() {
        let item = valid_config_item();
        let json = json!([item]);
        assert!(
            validate_json_simulation(&json, &JSON_SIMULATION_VALIDATOR),
            "Valid JSON should pass validation"
        );
    }

    #[test]
    fn metadata_round_trips_arbitrary_json_values() {
        let annotations = json!({
            "experiment": "capacidade — réplica",
            "replicate": 3,
            "enabled": true,
            "optional": null,
            "samples": [0.25, "label", false, null, {"nested": [1, 2]}],
            "graph": {"generator": "cycle", "vertices": 3},
            // An annotation with a parameter's name must remain opaque.
            "random_seed": "an annotation, not a seed"
        });
        let mut item = valid_config_item();
        item["metadata"] = annotations.clone();
        assert!(JSON_SIMULATION_VALIDATOR.is_valid(&json!([item.clone()])));
        let config: SimulationConfigurationItem = serde_json::from_value(item).unwrap();
        let serialized = serde_json::to_value(&config).unwrap();
        assert_eq!(serialized["metadata"], annotations);
        assert_eq!(config.random_seed, Some(42));
    }

    #[test]
    fn metadata_is_optional_and_defaults_to_an_empty_object() {
        for annotations in [None, Some(json!({}))] {
            let mut item = valid_config_item();
            if let Some(value) = annotations {
                item["metadata"] = value;
            }
            assert!(JSON_SIMULATION_VALIDATOR.is_valid(&json!([item.clone()])));
            let config: SimulationConfigurationItem = serde_json::from_value(item).unwrap();
            assert!(config.metadata.is_empty());
        }
    }

    #[test]
    fn metadata_rejects_non_objects_and_the_removed_field() {
        for invalid in [json!(null), json!([]), json!("text"), json!(3), json!(true)] {
            let mut item = valid_config_item();
            item["metadata"] = invalid;
            assert!(!JSON_SIMULATION_VALIDATOR.is_valid(&json!([item.clone()])));
            assert!(serde_json::from_value::<SimulationConfigurationItem>(item).is_err());
        }
    }

    #[test]
    fn test_multiple_valid_items() {
        let item1 = valid_config_item();
        let mut item2 = valid_config_item();
        item2["uuid"] = json!("123e4567-e89b-12d3-a456-426614174001");
        let json = json!([item1, item2]);
        assert!(
            validate_json_simulation(&json, &JSON_SIMULATION_VALIDATOR),
            "Multiple valid items should pass"
        );
    }

    #[test]
    fn test_graph_file_name_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("graph_file_name");
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing graph_file_name should fail"
        );

        let mut item = valid_config_item();
        item["graph_file_name"] = json!(123);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "graph_file_name as number should fail"
        );
    }

    #[test]
    fn test_initial_capacity_is_required() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("initial_capacity");
        assert!(!validate_json_simulation(
            &json!([item]),
            &JSON_SIMULATION_VALIDATOR
        ));
    }
    #[test]
    fn test_message_generation_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("message_generation");
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing message_generation should fail"
        );

        let mut item = valid_config_item();
        item["message_generation"] = json!(0);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "message_generation 0 should fail (exclusiveMinimum)"
        );

        let mut item = valid_config_item();
        item["message_generation"] = json!(-0.1);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "message_generation < 0 should fail"
        );

        let mut item = valid_config_item();
        item["message_generation"] = json!(1.1);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "message_generation > 1 should fail"
        );

        let mut item = valid_config_item();
        item["message_generation"] = json!(1);
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "message_generation 1 should pass"
        );
    }

    #[test]
    fn test_max_iterations_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("max_iterations");
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing max_iterations should fail"
        );

        let mut item = valid_config_item();
        item["max_iterations"] = json!(0);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "max_iterations 0 should fail (exclusiveMinimum)"
        );

        let mut item = valid_config_item();
        item["max_iterations"] = json!(10.5);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "max_iterations float should fail"
        );
    }

    #[test]
    fn test_warm_up_iterations_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("warm_up_iterations");
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing warm_up_iterations should pass"
        );

        let mut item = valid_config_item();
        item["warm_up_iterations"] = json!(-1);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "warm_up_iterations < 0 should fail"
        );

        let mut item = valid_config_item();
        item["warm_up_iterations"] = json!(0);
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "warm_up_iterations 0 should pass"
        );
    }

    #[test]
    fn test_random_seed_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("random_seed");
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing random_seed should pass"
        );

        let mut item = valid_config_item();
        item["random_seed"] = json!(-1);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "random_seed < 0 should fail"
        );
    }
    #[test]
    fn test_routing_method_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("routing_method");
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing routing_method should fail"
        );

        let valid_methods = vec![
            json!("minimal_paths"),
            json!("random_walk"),
            json!({ "limited_visibility": 0 }),
            json!({ "limited_visibility": 10 }),
        ];

        for method in valid_methods {
            let mut item = valid_config_item();
            item["routing_method"] = method.clone();
            assert!(
                validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
                "Valid routing_method {} should pass",
                method
            );
        }

        let invalid_methods = vec![
            json!("invalid_method"),
            json!({}),
            json!({ "limited_visibility": -1 }),
            json!({ "limited_visibility": 5, "extra": true }),
            json!(123),
        ];

        for method in invalid_methods {
            let mut item = valid_config_item();
            item["routing_method"] = method.clone();
            assert!(
                !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
                "Invalid routing_method {} should fail",
                method
            );
        }
    }

    #[test]
    fn test_observers_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("observers");
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing observers should pass"
        );

        let mut item = valid_config_item();
        item["observers"] = json!({});
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "observers as object should fail"
        );
    }

    #[test]
    fn test_modifier_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("modifier");
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing modifier should pass"
        );

        let mut item = valid_config_item();
        item["modifier"] = json!([]);
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "modifier as array should fail"
        );
    }

    fn capacity_modifier(momentum: serde_json::Value) -> serde_json::Value {
        json!({
            "type": "ModifierEdgeCapacity",
            "free_flow_rate": 0.5,
            "free_flow_sampling_time": 4,
            "momentum": momentum
        })
    }

    #[test]
    fn modifier_schema_accepts_valid_momentum_and_round_trips_it() {
        for momentum in [0.0, 0.25, 0.5, 0.9, 1.0 - f64::from(f32::EPSILON)] {
            let mut item = valid_config_item();
            item["modifier"] = capacity_modifier(json!(momentum));
            assert!(JSON_SIMULATION_VALIDATOR.is_valid(&json!([item.clone()])));
            let config: SimulationConfigurationItem = serde_json::from_value(item).unwrap();
            assert_eq!(config.modifier.as_ref().unwrap().momentum, momentum);
            assert_eq!(
                serde_json::to_value(config).unwrap()["modifier"]["momentum"],
                json!(momentum)
            );
        }
    }

    #[test]
    fn modifier_schema_rejects_momentum_outside_the_domain() {
        for momentum in [
            json!(-0.001),
            json!(1.0),
            json!(1.5),
            json!(null),
            json!("NaN"),
            json!("Infinity"),
        ] {
            let mut item = valid_config_item();
            item["modifier"] = capacity_modifier(momentum.clone());
            let errors: Vec<_> = JSON_SIMULATION_VALIDATOR
                .iter_errors(&json!([item]))
                .map(|error| error.instance_path().to_string())
                .collect();
            assert!(
                errors
                    .iter()
                    .any(|path| path == "/0/modifier" || path.starts_with("/0/modifier/")),
                "expected a modifier validation error for {momentum}, got {errors:?}"
            );
        }
    }
    #[test]
    fn modifier_schema_requires_momentum_when_a_modifier_is_present() {
        let mut item = valid_config_item();
        let mut modifier = capacity_modifier(json!(0.0));
        modifier.as_object_mut().unwrap().remove("momentum");
        item["modifier"] = modifier;
        assert!(!JSON_SIMULATION_VALIDATOR.is_valid(&json!([item])));
    }
}

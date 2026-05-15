include!(concat!(env!("OUT_DIR"), "/generated_types.rs"));

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
            "message_generation": 0.5,
            "max_iterations": 100,
            "warm_up_iterations": 10,
            "random_seed": 42,
            "routing_method": "minimal_paths",
            "observers": [],
            "modifiers": []
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
    fn test_modifiers_constraints() {
        let mut item = valid_config_item();
        item.as_object_mut().unwrap().remove("modifiers");
        assert!(
            validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "Missing modifiers should pass"
        );

        let mut item = valid_config_item();
        item["modifiers"] = json!({});
        assert!(
            !validate_json_simulation(&json!([item]), &JSON_SIMULATION_VALIDATOR),
            "modifiers as object should fail"
        );
    }
}

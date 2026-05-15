use std::env;
use std::fs;
use std::path::Path;
use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    let schema_path = "schema.json";
    println!("cargo:rerun-if-changed={}", schema_path);
    let content = fs::read_to_string(schema_path).expect("Failed to read schema.json");
    let schema = serde_json::from_str::<schemars::schema::RootSchema>(&content)
        .expect("Failed to parse schema JSON");

    let mut settings = TypeSpaceSettings::default();
    settings.with_struct_builder(true);

    let mut type_space = TypeSpace::new(&settings);
    type_space
        .add_root_schema(schema)
        .expect("Failed to add schema");

    let contents = format!("{}", type_space.to_stream().to_string());
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_types.rs");
    fs::write(&dest_path, contents).expect("Failed to write generated types");
}

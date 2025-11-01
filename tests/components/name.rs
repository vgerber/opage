use std::path::PathBuf;

use opage::{parser::component::generate_components, utils::config::Config};

#[test]
fn title_of_component_used() {
    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/components/specs/component_with_title.openapi.yaml");

    let yaml = std::fs::read_to_string(spec_file_path).expect("Failed to read yaml");

    let spec = oas3::from_yaml(yaml).expect("Failed to read spec");
    let config = Config::new();

    let object_database = generate_components(&spec, &config).unwrap();
    assert_eq!(
        vec!["ValidName"],
        object_database.keys().collect::<Vec<&String>>()
    );
}

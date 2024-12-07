use std::path::PathBuf;

use opage::{generator::component::generate_components, utils::config::Config};

#[test]
fn empty_component() {
    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/components/specs/empty_component.openapi.yaml");

    let spec = oas3::from_path(spec_file_path).expect("Failed to read spec");
    let config = Config::new();

    let object_database = generate_components(&spec, &config).unwrap();
    assert_eq!(
        vec!["Empty"],
        object_database.keys().collect::<Vec<&String>>()
    );
}

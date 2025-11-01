use std::path::PathBuf;

use opage::{
    parser::component::{
        generate_components,
        object_definition::types::{ModuleInfo, ObjectDefinition},
    },
    utils::config::Config,
};

#[test]
fn empty_component() {
    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/components/specs/empty_component.openapi.yaml");

    let yaml = std::fs::read_to_string(spec_file_path).expect("Failed to read yaml");
    let spec = oas3::from_yaml(yaml).expect("Failed to read spec");
    let config = Config::new();

    let object_database = generate_components(&spec, &config).unwrap();
    assert_eq!(
        vec!["Empty"],
        object_database.keys().collect::<Vec<&String>>()
    );
}

#[test]
fn self_ref_component() {
    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/components/specs/self_ref.openapi.yaml");

    let yaml = std::fs::read_to_string(spec_file_path).expect("Failed to read yaml");
    let spec = oas3::from_yaml(yaml).expect("Failed to read spec");
    let config = Config::new();

    let object_database = generate_components(&spec, &config).unwrap();
    assert!(object_database.contains_key("ConfigurationResourceArray"));
    assert!(object_database.contains_key("ConfigurationResource"));
    assert!(object_database.contains_key("ConfigurationResourceId"));

    let configuration_resource = match object_database.get("ConfigurationResource").unwrap() {
        ObjectDefinition::Struct(struct_definition) => struct_definition,
        _ => panic!("Expected a struct"),
    };

    assert_eq!(
        Vec::<ModuleInfo>::new(),
        configuration_resource.used_modules
    );

    assert_eq!(
        Vec::<&ModuleInfo>::new(),
        configuration_resource.get_required_modules()
    );
}

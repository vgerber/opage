use opage::{
    generator::rust_reqwest_async::path::http_request::generate_operation,
    parser::component::object_definition::types::ObjectDatabase,
    utils::{log::Logger, name_mapping::NameMapping},
};
use reqwest::Method;
use std::path::PathBuf;

static LOGGER: Logger = Logger;

#[test]
fn empty_json() {
    log::set_logger(&LOGGER).expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Trace);

    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/response/specs/empty_json.openapi.yaml");

    let yaml = std::fs::read_to_string(spec_file_path).expect("Failed to read spec yaml");
    let spec = oas3::from_yaml(yaml).expect("Failed to read spec");
    let path_spec = spec.paths.as_ref().unwrap().get("/test").unwrap();

    let mut object_database = ObjectDatabase::new();
    let name_mapping = NameMapping::new();

    generate_operation(
        &spec,
        &name_mapping,
        &Method::POST,
        "/test",
        &path_spec.post.as_ref().unwrap(),
        &mut object_database,
    )
    .expect("Failed to generated path");
}

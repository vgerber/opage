use opage::{
    generator::{component::ObjectDatabase, path::default_request::generate_operation},
    utils::name_mapping::NameMapping,
};
use reqwest::Method;
use std::path::PathBuf;

#[test]
fn empty_json() {
    let mut spec_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_file_path.push("tests/response/specs/empty_json.openapi.yaml");

    let spec = oas3::from_path(spec_file_path).expect("Failed to read spec");
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
    .unwrap();
}

pub mod cli;
pub mod generator;
pub mod utils;

use std::{fs::File, io::Write, path::Path};

use cli::cli;
use generator::{
    component::{generate_components, write_struct_database},
    paths::generate_paths,
};
use utils::name_mapper::NameMapper;

fn main() {
    let matches = cli().get_matches();

    let output_dir = matches
        .get_one::<String>("output-dir")
        .map(String::as_str)
        .expect("output-dir missing");
    let spec_file_path = matches
        .get_one::<String>("spec")
        .map(String::as_str)
        .expect("spec missing");
    let name_mapping_file_path = matches
        .get_one::<String>("name-mapping")
        .map(String::as_str);

    // Start generating

    // 1. Read spec
    let spec = match oas3::from_path(Path::new(spec_file_path)) {
        Ok(spec) => spec,
        Err(err) => panic!("{}", err.to_string()),
    };

    // 2. Get mapper for invalid language names
    let name_mapper = match name_mapping_file_path {
        Some(mapping_file) => {
            NameMapper::from(Path::new(mapping_file)).expect("Mapping json not found")
        }
        None => NameMapper::new(),
    };

    // 3. Generate Code
    // 3.1 Components and database for type referencing
    let struct_database = &mut generate_components(&spec, &name_mapper).unwrap();

    // 3.2 Generate paths requests
    generate_paths(&spec, struct_database, &name_mapper);

    // 3.3 Write all registered objects to individual type definitions
    if let Err(err) = write_struct_database(struct_database, &name_mapper) {
        panic!("{}", err)
    }

    // 4. Project setup
    let mut lib_file =
        File::create(format!("{}/src/lib.rs", output_dir)).expect("Failed to create lib.rs");

    lib_file
        .write("pub mod objects;\n".to_string().as_bytes())
        .unwrap();
    lib_file
        .write("pub mod paths;\n".to_string().as_bytes())
        .unwrap();
}

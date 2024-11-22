pub mod generator;
pub mod utils;

use std::{fs::File, io::Write, path::Path};

use generator::{
    component::{generate_components, write_struct_database},
    paths::generate_paths,
};
use utils::name_mapper::NameMapper;

fn main() {
    let wandelbots_spec = oas3::from_path(Path::new("wandelbots.openapi.yaml")).unwrap();
    let name_mapper =
        NameMapper::new(Path::new("name_mapping.json")).expect("Mapping json not found");

    let schemas_length = wandelbots_spec.components.as_ref().unwrap().schemas.len();
    let struct_database = &mut generate_components(&wandelbots_spec, &name_mapper).unwrap();
    generate_paths(&wandelbots_spec, struct_database, &name_mapper);

    write_struct_database(struct_database, &name_mapper).unwrap();

    let mut lib_file = File::create("output/src/lib.rs").expect("Failed to create lib.rs");

    lib_file
        .write("pub mod objects;\n".to_string().as_bytes())
        .unwrap();
    lib_file
        .write("pub mod paths;\n".to_string().as_bytes())
        .unwrap();

    println!("Components {}", schemas_length);
    println!("Hello, world!");
}

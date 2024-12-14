use std::{
    fs::{self, File},
    io::Write,
};

use log::{error, info, trace};
use oas3::Spec;
use object_definition::{
    generate_object, get_components_base_path, get_object_name, modules_to_string,
    types::{ObjectDatabase, ObjectDefinition},
};

use crate::utils::{config::Config, name_mapping::NameMapping};

pub mod object_definition;
pub mod type_definition;

pub fn generate_components(spec: &Spec, config: &Config) -> Result<ObjectDatabase, String> {
    let components = match spec.components {
        Some(ref components) => components,
        None => return Ok(ObjectDatabase::new()),
    };

    let mut object_database = ObjectDatabase::new();

    for (component_name, object_ref) in &components.schemas {
        if config.ignore.component_ignored(&component_name) {
            info!("\"{}\" ignored", component_name);
            continue;
        }

        info!("Generating component \"{}\"", component_name);

        let resolved_object = match object_ref.resolve(spec) {
            Ok(object) => object,
            Err(err) => {
                error!(
                    "Unable to parse component {} {}",
                    component_name,
                    err.to_string()
                );
                continue;
            }
        };

        let definition_path = get_components_base_path();
        let object_name = match resolved_object.title {
            Some(ref title) => config
                .name_mapping
                .name_to_struct_name(&definition_path, &title),
            None => config
                .name_mapping
                .name_to_struct_name(&definition_path, &component_name),
        };

        if object_database.contains_key(&object_name) {
            info!(
                "Component \"{}\" already found in database and will be skipped",
                object_name
            );
            continue;
        }

        let object_definition = match generate_object(
            spec,
            &mut object_database,
            definition_path,
            &object_name,
            &resolved_object,
            &config.name_mapping,
        ) {
            Ok(object_definition) => object_definition,
            Err(err) => {
                error!("{} {}\n", component_name, err);
                continue;
            }
        };

        if let ObjectDefinition::Primitive(type_definition) = object_definition {
            trace!(
                "Primitive object {} will not be added to database",
                type_definition.name
            );
            continue;
        }

        let object_name = get_object_name(&object_definition);

        match object_database.contains_key(object_name) {
            true => {
                error!("ObjectDatabase already contains an object {}", object_name);
                continue;
            }
            _ => {
                trace!("Adding component/struct {} to database", object_name);
                object_database.insert(object_name.clone(), object_definition);
            }
        }
    }

    Ok(object_database)
}

pub fn write_object_database(
    output_dir: &str,
    object_database: &ObjectDatabase,
    name_mapping: &NameMapping,
) -> Result<(), String> {
    fs::create_dir_all(format!("{}/src/objects/", output_dir))
        .expect("Creating objects dir failed");

    for (_, object_definition) in object_database {
        let object_name = get_object_name(object_definition);

        let module_name = name_mapping.name_to_module_name(object_name);

        let mut object_file =
            match File::create(format!("{}/src/objects/{}.rs", output_dir, module_name)) {
                Ok(file) => file,
                Err(err) => {
                    error!(
                        "Unable to create file {}.rs {}",
                        module_name,
                        err.to_string()
                    );
                    continue;
                }
            };

        match object_definition {
            ObjectDefinition::Struct(struct_definition) => {
                object_file
                    .write(modules_to_string(&struct_definition.get_required_modules()).as_bytes())
                    .expect("Failed to write imports");
                object_file.write("\n".as_bytes()).unwrap();

                object_file
                    .write(struct_definition.to_string(true).as_bytes())
                    .unwrap();
            }
            ObjectDefinition::Enum(enum_definition) => {
                object_file
                    .write(modules_to_string(&enum_definition.get_required_modules()).as_bytes())
                    .expect("Failed to write imports");
                object_file.write("\n".as_bytes()).unwrap();

                object_file
                    .write(enum_definition.to_string(true).as_bytes())
                    .unwrap();
            }
            ObjectDefinition::Primitive(type_definition) => {
                object_file
                    .write(
                        modules_to_string(
                            &type_definition
                                .module
                                .as_ref()
                                .map_or(vec![], |module| vec![module]),
                        )
                        .as_bytes(),
                    )
                    .expect("Failed to write imports");
                object_file.write("\n".as_bytes()).unwrap();

                object_file
                    .write(format!("type {};\n", type_definition.name).as_bytes())
                    .unwrap();
            }
        }
    }

    let mut object_mod_file = match File::create(format!("{}/src/objects/mod.rs", output_dir)) {
        Ok(file) => file,
        Err(err) => {
            return Err(format!(
                "Unable to create file {} {}",
                format!("{}/src/objects/mod.rs", output_dir),
                err.to_string()
            ))
        }
    };

    for (struct_name, _) in object_database {
        match object_mod_file.write(
            format!(
                "pub mod {};\n",
                name_mapping.name_to_module_name(struct_name)
            )
            .to_string()
            .as_bytes(),
        ) {
            Ok(_) => (),
            Err(err) => return Err(format!("Failed to write to mod {}", err.to_string())),
        }
    }
    Ok(())
}

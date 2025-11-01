use log::{error, info, trace, warn};
use oas3::Spec;
use object_definition::{
    generate_object, get_components_base_path, get_object_name, types::ObjectDatabase,
};

use crate::utils::config::Config;

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

        let object_name = get_object_name(&object_definition);

        match object_database.contains_key(object_name) {
            true => {
                warn!("ObjectDatabase already contains an object {}. This might be caused by cyclic references", object_name);
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

use log::trace;
use oas3::{
    spec::{ObjectSchema, SchemaTypeSet},
    Spec,
};

use crate::utils::name_mapping::NameMapping;

use super::{
    object_definition::{
        get_object_name, get_object_or_ref_struct_name, get_or_create_object,
        types::{ModuleInfo, TypeDefinition},
    },
    ObjectDatabase,
};

pub fn get_type_from_schema(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    if let Some(ref schema_type) = object_schema.schema_type {
        return get_type_from_schema_type(
            spec,
            object_database,
            definition_path,
            schema_type,
            object_schema,
            object_variable_fallback_name,
            name_mapping,
        );
    }

    if object_schema.any_of.len() > 0 {
        return get_type_from_any_type(
            spec,
            object_database,
            definition_path,
            object_schema,
            object_variable_fallback_name,
            name_mapping,
        );
    }

    let empty_object_name = match object_variable_fallback_name {
        Some(empty_object_name) => empty_object_name,
        None => return Err("Cannot create empty object without name".to_owned()),
    };

    // empty type
    match get_or_create_object(
        spec,
        object_database,
        definition_path,
        empty_object_name,
        object_schema,
        name_mapping,
    ) {
        Ok(object_definition) => {
            let object_name = get_object_name(&object_definition);
            Ok(TypeDefinition {
                name: object_name.clone(),
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapping.name_to_module_name(&object_name)
                    ),
                    name: object_name.clone(),
                }),
            })
        }
        Err(err) => Err(err),
    }
}

pub fn get_type_from_any_type(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    let object_variable_name = match object_schema.title {
        Some(ref title) => &name_mapping.name_to_struct_name(&definition_path, &title),
        None => match object_variable_fallback_name {
            Some(title_fallback) => title_fallback,
            None => {
                return Err(format!(
                    "Cannot fetch type because no title or title_fallback was given"
                ))
            }
        },
    };

    trace!("Generating any_type {}", object_variable_name);

    let object_definition = match get_or_create_object(
        spec,
        object_database,
        definition_path,
        &object_variable_name,
        &object_schema,
        name_mapping,
    ) {
        Ok(object_definition) => object_definition,
        Err(err) => {
            return Err(format!(
                "Failed to generated struct {} {}",
                object_variable_name, err
            ));
        }
    };

    let object_name = get_object_name(&object_definition);

    Ok(TypeDefinition {
        name: object_name.clone(),
        module: Some(ModuleInfo {
            path: format!(
                "crate::objects::{}",
                name_mapping.name_to_module_name(&object_name)
            ),
            name: object_name.clone(),
        }),
    })
}

pub fn get_type_from_schema_type(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    schema_type: &SchemaTypeSet,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    let single_type = match schema_type {
        oas3::spec::SchemaTypeSet::Single(single_type) => single_type,
        _ => return Err(format!("MultiType is not supported")),
    };

    let object_variable_name = match object_schema.title {
        Some(ref title) => title,
        None => match object_variable_fallback_name {
            Some(title_fallback) => title_fallback,
            None => {
                return Err(format!(
                    "Cannot fetch type because no title or title_fallback was given {:#?}",
                    object_schema
                ))
            }
        },
    };

    match single_type {
        oas3::spec::SchemaType::Boolean => Ok(TypeDefinition {
            name: "bool".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::String => Ok(TypeDefinition {
            name: "String".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Number => Ok(TypeDefinition {
            name: "f64".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Integer => Ok(TypeDefinition {
            name: "i32".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Array => {
            let item_object_ref = match object_schema.items {
                Some(ref item_object) => item_object,
                None => return Err(format!("Array has no item type")),
            };

            let (item_type_definition_path, item_type_name) = match get_object_or_ref_struct_name(
                spec,
                &definition_path,
                name_mapping,
                &item_object_ref,
            ) {
                Ok(definition_path_and_name) => definition_path_and_name,
                Err(err) => return Err(format!("Unable to determine ArrayItem type name {}", err)),
            };

            let item_object = match item_object_ref.resolve(spec) {
                Ok(item_object) => item_object,
                Err(err) => {
                    return Err(format!(
                        "Failed to resolve ArrayItem\n{:#?}\n{}",
                        item_object_ref,
                        err.to_string()
                    ))
                }
            };

            match get_type_from_schema(
                spec,
                object_database,
                item_type_definition_path,
                &item_object,
                Some(&item_type_name),
                name_mapping,
            ) {
                Ok(mut type_definition) => {
                    type_definition.name = format!("Vec<{}>", type_definition.name);
                    return Ok(type_definition);
                }
                Err(err) => Err(err),
            }
        }
        oas3::spec::SchemaType::Object => {
            let object_definition = match get_or_create_object(
                spec,
                object_database,
                definition_path,
                &object_variable_name,
                &object_schema,
                name_mapping,
            ) {
                Ok(object_definition) => object_definition,
                Err(err) => {
                    return Err(format!(
                        "Failed to generated struct {} {}",
                        object_variable_name, err
                    ));
                }
            };

            let object_name = get_object_name(&object_definition);

            Ok(TypeDefinition {
                name: object_name.clone(),
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapping.name_to_module_name(&object_name)
                    ),
                    name: object_name.clone(),
                }),
            })
        }
        _ => Err(format!("Type {:?} not supported", single_type)),
    }
}

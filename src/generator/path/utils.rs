use std::collections::{BTreeMap, HashMap};

use log::{trace, warn};
use oas3::{
    spec::{ObjectOrReference, ObjectSchema, RequestBody, Response},
    Spec,
};
use reqwest::StatusCode;

use crate::{
    generator::component::{
        object_definition::{
            get_object_or_ref_struct_name, is_object_empty,
            types::{ModuleInfo, ObjectDatabase, TypeDefinition},
        },
        type_definition::get_type_from_schema,
    },
    utils::name_mapping::NameMapping,
};

pub fn is_path_parameter(path_component: &str) -> bool {
    path_component.starts_with("{") && path_component.ends_with("}")
}

#[derive(Clone, Debug)]
pub enum TransferMediaType {
    ApplicationJson(Option<TypeDefinition>),
}

#[derive(Clone, Debug)]
pub struct ResponseEntity {
    pub canonical_status_code: String,
    pub content: Option<TransferMediaType>,
}

#[derive(Clone, Debug)]
pub struct RequestEntity {
    pub content: TransferMediaType,
}

pub type ResponseEntities = HashMap<String, ResponseEntity>;

fn parse_json_data(
    spec: &Spec,
    definition_path: Vec<String>,
    name_mapping: &NameMapping,
    new_object_name: &str,
    object_database: &mut ObjectDatabase,
    json_schema_object_or_ref: &ObjectOrReference<ObjectSchema>,
) -> Result<Option<TypeDefinition>, String> {
    let is_json_object_empty = match json_schema_object_or_ref.resolve(spec) {
        Ok(schema_object) => is_object_empty(&schema_object),
        Err(err) => {
            return Err(format!(
                "Failed to resolve json response {}",
                err.to_string()
            ));
        }
    };

    if is_json_object_empty {
        return Ok(None);
    }

    let json_object_definition_opt = match json_schema_object_or_ref {
        ObjectOrReference::Ref { ref_path: _ } => match get_object_or_ref_struct_name(
            spec,
            &definition_path,
            name_mapping,
            &json_schema_object_or_ref,
        ) {
            Ok((_, object_name)) => Some(TypeDefinition {
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapping.name_to_module_name(&object_name)
                    ),
                    name: object_name.clone(),
                }),
                name: object_name.clone(),
            }),
            Err(err) => {
                return Err(format!(
                    "Unable to determine response type ref name {}",
                    err
                ))
            }
        },
        ObjectOrReference::Object(object_schema) => match get_type_from_schema(
            spec,
            object_database,
            definition_path.clone(),
            &object_schema,
            Some(new_object_name),
            name_mapping,
        ) {
            Ok(type_definition) => Some(type_definition),
            Err(err) => return Err(err),
        },
    };

    match json_object_definition_opt {
        Some(json_object_definition) => Ok(Some(json_object_definition)),
        None => return Err(format!("JsonObjectName not found")),
    }
}

pub fn generate_request_body(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    name_mapping: &NameMapping,
    request_body: &ObjectOrReference<RequestBody>,
    function_name: &str,
) -> Result<RequestEntity, String> {
    let request = match request_body.resolve(spec) {
        Ok(request) => request,
        Err(err) => {
            return Err(format!(
                "Failed to resolve request body {}",
                err.to_string()
            ))
        }
    };

    if request.content.len() > 1 {
        warn!("Only a single json object is supported");
    }

    let json_data = match request.content.get("application/json") {
        Some(json_data) => json_data,
        None => return Err("No json payload found".to_string()),
    };

    let json_schema_object_or_ref = match json_data.schema {
        Some(ref schema) => schema,
        None => return Err(format!("Failed to parse response json data",)),
    };

    let json_object = match parse_json_data(
        spec,
        definition_path.clone(),
        name_mapping,
        &name_mapping
            .name_to_struct_name(&definition_path, &format!("{}RequestBody", &function_name)),
        object_database,
        json_schema_object_or_ref,
    ) {
        Ok(json_object) => json_object,
        Err(err) => return Err(err),
    };

    let json_object_type_definition = match json_object {
        Some(json_object) => json_object,
        None => {
            trace!("{} empty json request body object skipped", function_name);
            return Ok(RequestEntity {
                content: TransferMediaType::ApplicationJson(None),
            });
        }
    };

    Ok(RequestEntity {
        content: TransferMediaType::ApplicationJson(Some(json_object_type_definition)),
    })
}

pub fn generate_responses(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    name_mapping: &NameMapping,
    responses: &BTreeMap<String, Response>,
    function_name: &str,
) -> Result<ResponseEntities, String> {
    let mut response_entities = ResponseEntities::new();
    for (response_key, response) in responses {
        trace!("Generate response {}", response_key);
        if response_key == "default" {
            continue;
        }

        let canonical_status_code = match StatusCode::from_bytes(response_key.as_bytes()) {
            Ok(status_code) => match name_mapping.status_code_to_canonical_name(status_code) {
                Ok(canonical_status_code) => canonical_status_code,
                Err(err) => return Err(err),
            },
            Err(err) => {
                return Err(format!(
                    "Failed to parse status code {} {}",
                    response_key,
                    err.to_string()
                ))
            }
        };

        if response.content.len() > 1 {
            warn!("Only a single json object is supported");
        }

        if response.content.len() == 0 {
            response_entities.insert(
                response_key.clone(),
                ResponseEntity {
                    canonical_status_code: canonical_status_code.to_owned(),
                    content: None,
                },
            );
            continue;
        }

        let json_data = match response.content.get("application/json") {
            Some(json_data) => json_data,
            None => continue,
        };

        let json_schema_object_or_ref = match json_data.schema {
            Some(ref schema) => schema,
            None => return Err(format!("Failed to parse response json data",)),
        };

        let json_object = match parse_json_data(
            spec,
            definition_path.clone(),
            name_mapping,
            &name_mapping.name_to_struct_name(
                &definition_path,
                &format!("{}{}", &function_name, &canonical_status_code),
            ),
            object_database,
            json_schema_object_or_ref,
        ) {
            Ok(json_object) => json_object,
            Err(err) => return Err(err),
        };

        response_entities.insert(
            response_key.clone(),
            ResponseEntity {
                canonical_status_code: canonical_status_code.to_owned(),
                content: Some(TransferMediaType::ApplicationJson(json_object)),
            },
        );
    }
    Ok(response_entities)
}

pub fn use_module_to_string(module: &ModuleInfo) -> String {
    if module.path.is_empty() {
        return format!("use {};", module.name);
    }
    format!("use {}::{};", module.path, module.name)
}

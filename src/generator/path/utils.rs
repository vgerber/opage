use std::collections::{BTreeMap, HashMap};

use log::{trace, warn};
use oas3::{
    spec::{ObjectOrReference, RequestBody, Response},
    Spec,
};
use reqwest::StatusCode;

use crate::{
    generator::component::{
        get_object_or_ref_struct_name, get_type_from_schema, ModuleInfo, ObjectDatabase,
        TypeDefinition,
    },
    utils::name_mapping::NameMapping,
};

pub fn is_path_parameter(path_component: &str) -> bool {
    path_component.starts_with("{") && path_component.ends_with("}")
}

#[derive(Clone, Debug)]
pub enum TransferMediaType {
    ApplicationJson(TypeDefinition),
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
    let json_object_definition_opt = match json_data.schema {
        Some(ref object_or_ref) => match object_or_ref {
            ObjectOrReference::Ref { ref_path: _ } => match get_object_or_ref_struct_name(
                spec,
                &definition_path,
                name_mapping,
                object_or_ref,
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
                object_schema,
                Some(&format!(
                    "{}RequestBody",
                    name_mapping
                        .name_to_struct_name(&definition_path, &function_name)
                        .to_owned(),
                )),
                name_mapping,
            ) {
                Ok(type_definition) => Some(type_definition),
                Err(err) => return Err(err),
            },
        },
        None => return Err(format!("Failed to parse response json data",)),
    };

    let json_object_definition = match json_object_definition_opt {
        Some(json_object_definition) => json_object_definition,
        None => return Err(format!("JsonObjectName not found")),
    };

    Ok(RequestEntity {
        content: TransferMediaType::ApplicationJson(json_object_definition),
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
        let json_object_definition_opt = match json_data.schema {
            Some(ref object_or_ref) => match object_or_ref {
                ObjectOrReference::Ref { ref_path: _ } => match get_object_or_ref_struct_name(
                    spec,
                    &definition_path,
                    name_mapping,
                    object_or_ref,
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
                    object_schema,
                    Some(&format!(
                        "{}{}",
                        name_mapping
                            .name_to_struct_name(&definition_path, &function_name)
                            .to_owned(),
                        canonical_status_code
                    )),
                    name_mapping,
                ) {
                    Ok(type_definition) => Some(type_definition),
                    Err(err) => return Err(err),
                },
            },
            None => return Err(format!("Failed to parse response json data",)),
        };

        let json_object_definition = match json_object_definition_opt {
            Some(json_object_definition) => json_object_definition,
            None => return Err(format!("JsonObjectName not found")),
        };

        response_entities.insert(
            response_key.clone(),
            ResponseEntity {
                canonical_status_code: canonical_status_code.to_owned(),
                content: Some(TransferMediaType::ApplicationJson(json_object_definition)),
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

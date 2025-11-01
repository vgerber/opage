use super::utils::{
    generate_request_body, generate_responses, is_path_parameter, TransferMediaType,
};
use crate::generator::rust_reqwest_async::templates::{
    EnumDefinitionTemplate, PrimitiveDefinitionTemplate, StructDefinitionTemplate,
};
use crate::{
    parser::component::{
        object_definition::{
            oas3_type_to_string,
            types::{
                ModuleInfo, ObjectDatabase, PropertyDefinition, StructDefinition, TypeDefinition,
            },
        },
        type_definition::get_type_from_schema,
    },
    utils::name_mapping::NameMapping,
};
use askama::Template;
use log::error;
use oas3::{
    spec::{FromRef, ObjectOrReference, ObjectSchema, Operation, ParameterIn},
    Spec,
};
use std::collections::HashMap;

#[derive(Debug)]
struct QueryParameter {
    is_required: bool,
    is_array: bool,
    real_name: String,
    name: String,
    struct_name: String,
}

#[derive(Debug)]
struct FunctionParameter {
    name: String,
    type_name: String,
}

#[derive(Template)]
#[template(path = "rust_reqwest_async/websocket.rs.jinja", ext = "rs")]
struct WebSocketRequestTemplate {
    // Base
    module_imports: Vec<ModuleInfo>,
    struct_definitions: Vec<StructDefinitionTemplate>,
    enum_definitions: Vec<EnumDefinitionTemplate>,
    primitive_definitions: Vec<PrimitiveDefinitionTemplate>,
    // WebSocket
    socket_stream_struct_name: String,
    response_type_name: String,
    function_name: String,
    function_parameters: Vec<FunctionParameter>,
    path_format_string: String,
    path_parameter_arguments: String,
    query_parameters_mutable: bool,
    query_parameters: Vec<QueryParameter>,
}

pub fn generate_operation(
    spec: &Spec,
    name_mapping: &NameMapping,
    path: &str,
    operation: &Operation,
    object_database: &mut ObjectDatabase,
) -> Result<String, String> {
    let operation_definition_path: Vec<String> = vec![path.to_owned()];

    let function_name = match operation.operation_id {
        Some(ref operation_id) => name_mapping.name_to_module_name(operation_id),
        None => return Err("No operation_id found".to_owned()),
    };

    let response_entities = match generate_responses(
        spec,
        object_database,
        &operation_definition_path,
        name_mapping,
        &operation.responses(spec),
        &function_name,
    ) {
        Ok(response_entities) => response_entities,
        Err(err) => return Err(err),
    };

    let socket_transferred_media_type = match response_entities.get("200") {
        Some(ok_response) => {
            let mut socket_transferred_media_type = None;
            for (_, transfer_media_type) in &ok_response.content {
                socket_transferred_media_type = Some(transfer_media_type);
                break;
            }

            match socket_transferred_media_type {
                Some(socket_transferred_media_type) => socket_transferred_media_type,
                None => return Err("Transfer type missing".to_owned()),
            }
        }
        None => return Err("No OK response found".to_owned()),
    };

    let socket_transfer_type_definition = match socket_transferred_media_type {
        TransferMediaType::ApplicationJson(type_definition) => match type_definition {
            Some(type_definition) => type_definition,
            None => {
                return Err(format!(
                    "Websocket with empty response body is not supported"
                ))
            }
        },
        TransferMediaType::TextPlain => &TypeDefinition {
            name: oas3_type_to_string(&oas3::spec::SchemaType::String),
            module: None,
        },
    };

    let path_parameters_struct_name = format!(
        "{}PathParameters",
        name_mapping.name_to_struct_name(&operation_definition_path, &function_name)
    );
    let mut path_parameters_definition_path = operation_definition_path.clone();
    path_parameters_definition_path.push(path_parameters_struct_name.clone());

    let path_parameters_ordered = path
        .split("/")
        .filter(|&path_component| is_path_parameter(&path_component))
        .map(|path_component| path_component.replace("{", "").replace("}", ""))
        .map(|path_component| PropertyDefinition {
            module: None,
            name: name_mapping
                .name_to_property_name(&path_parameters_definition_path, &path_component),
            real_name: path_component,
            required: true,
            type_name: "&str".to_owned(),
        })
        .collect::<Vec<PropertyDefinition>>();
    let path_struct_definition = StructDefinition {
        name: path_parameters_struct_name,
        used_modules: vec![],
        properties: path_parameters_ordered
            .iter()
            .map(|path_component| {
                (
                    path_component.name.clone(),
                    PropertyDefinition {
                        module: None,
                        name: path_component.name.clone(),
                        real_name: path_component.real_name.clone(),
                        required: path_component.required,
                        type_name: "String".to_owned(),
                    },
                )
            })
            .collect::<HashMap<String, PropertyDefinition>>(),
        local_objects: HashMap::new(),
    };
    let mut struct_definitions = vec![&path_struct_definition];

    let path_format_string = path
        .split("/")
        .map(|path_component| {
            return match is_path_parameter(path_component) {
                true => String::from("{}"),
                _ => path_component.to_owned(),
            };
        })
        .collect::<Vec<String>>()
        .join("/");

    let mut function_parameters: Vec<FunctionParameter> = vec![];

    if !path_struct_definition.properties.is_empty() {
        function_parameters.push(FunctionParameter {
            name: name_mapping
                .name_to_property_name(&operation_definition_path, &path_struct_definition.name),
            type_name: path_struct_definition.name.clone(),
        });
    }

    let mut module_imports = vec![
        ModuleInfo {
            name: "TcpStream".to_owned(),
            path: "std::net".to_owned(),
        },
        ModuleInfo {
            name: "connect".to_owned(),
            path: "tungstenite".to_owned(),
        },
        ModuleInfo {
            name: "Error".to_owned(),
            path: "tungstenite".to_owned(),
        },
        ModuleInfo {
            name: "WebSocket".to_owned(),
            path: "tungstenite".to_owned(),
        },
        ModuleInfo {
            name: "CloseFrame".to_owned(),
            path: "tungstenite::protocol".to_owned(),
        },
        ModuleInfo {
            name: "MaybeTlsStream".to_owned(),
            path: "tungstenite::stream".to_owned(),
        },
        ModuleInfo {
            name: "Uri".to_owned(),
            path: "tungstenite::http".to_owned(),
        },
        ModuleInfo {
            name: "IntoClientRequest".to_owned(),
            path: "tungstenite::client".to_owned(),
        },
        ModuleInfo {
            name: "HeaderName".to_owned(),
            path: "tungstenite::http".to_owned(),
        },
    ];

    if let Some(ref socket_transfer_type_module) = socket_transfer_type_definition.module {
        module_imports.push(socket_transfer_type_module.clone());
    }

    // Query params
    let mut query_struct = StructDefinition {
        name: format!(
            "{}QueryParameters",
            name_mapping.name_to_struct_name(&operation_definition_path, &function_name)
        ),
        properties: HashMap::new(),
        used_modules: vec![],
        local_objects: HashMap::new(),
    };
    let mut query_operation_definition_path = operation_definition_path.clone();
    query_operation_definition_path.push(query_struct.name.clone());

    for parameter_ref in &operation.parameters {
        let parameter = match parameter_ref.resolve(spec) {
            Ok(parameter) => parameter,
            Err(err) => return Err(format!("Failed to resolve parameter {}", err.to_string())),
        };
        if parameter.location != ParameterIn::Query {
            continue;
        }

        let parameter_type = match parameter.schema {
            Some(schema) => match schema {
                ObjectOrReference::Object(object_schema) => get_type_from_schema(
                    spec,
                    object_database,
                    query_operation_definition_path.clone(),
                    &object_schema,
                    Some(&parameter.name),
                    name_mapping,
                ),
                ObjectOrReference::Ref { ref_path } => {
                    match ObjectSchema::from_ref(spec, &ref_path) {
                        Ok(object_schema) => get_type_from_schema(
                            spec,
                            object_database,
                            vec![],
                            &object_schema,
                            Some(&parameter.name),
                            name_mapping,
                        ),
                        Err(err) => {
                            return Err(format!(
                                "Failed to resolve parameter {} {}",
                                parameter.name,
                                err.to_string()
                            ))
                        }
                    }
                }
            },
            None => return Err(format!("Parameter {} has no schema", parameter.name)),
        };

        let _ = match parameter_type {
            Ok(parameter_type) => query_struct.properties.insert(
                name_mapping
                    .name_to_property_name(&query_operation_definition_path, &parameter.name),
                PropertyDefinition {
                    name: name_mapping
                        .name_to_property_name(&query_operation_definition_path, &parameter.name),
                    module: parameter_type.module,
                    real_name: parameter.name,
                    required: match parameter.required {
                        Some(required) => required,
                        None => false,
                    },
                    type_name: parameter_type.name,
                },
            ),
            Err(err) => return Err(err),
        };
    }

    if query_struct.properties.len() > 0 {
        function_parameters.push(FunctionParameter {
            name: name_mapping
                .name_to_property_name(&operation_definition_path, &query_struct.name),
            type_name: query_struct.name.clone(),
        });
        struct_definitions.push(&query_struct);
    }

    function_parameters.push(FunctionParameter {
        name: "additional_headers".to_owned(),
        type_name: "Option<Vec<(String, String)>>".to_owned(),
    });

    // Request Body
    let request_body = match operation.request_body {
        Some(ref request_body) => {
            match generate_request_body(
                spec,
                object_database,
                &operation_definition_path,
                name_mapping,
                request_body,
                &function_name,
            ) {
                Ok(request_body) => Some(request_body),
                Err(err) => {
                    return Err(format!(
                        "Failed to generated request body {}",
                        err.to_string()
                    ))
                }
            }
        }
        None => None,
    };

    if let Some(ref request_body) = request_body {
        if request_body.content.len() > 1 {
            error!("RequestBody with multiple content types is not supported")
        }

        for (_, transfer_media_type) in &request_body.content {
            match transfer_media_type {
                TransferMediaType::ApplicationJson(ref type_definition) => match type_definition {
                    Some(ref type_definition) => {
                        if let Some(ref module) = type_definition.module {
                            if !module_imports.contains(module) {
                                module_imports.push(module.clone());
                            }
                        }
                        function_parameters.push(FunctionParameter {
                            name: name_mapping.name_to_property_name(
                                &operation_definition_path,
                                &type_definition.name,
                            ),
                            type_name: type_definition.name.clone(),
                        });
                    }
                    None => (),
                },
                TransferMediaType::TextPlain => function_parameters.push(FunctionParameter {
                    name: "request_string".to_owned(),
                    type_name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                }),
            }
            break;
        }
    }

    let mut path_parameter_arguments = path_parameters_ordered
        .iter()
        .map(|parameter| {
            format!(
                "{}.{}",
                name_mapping.name_to_property_name(
                    &operation_definition_path,
                    &path_struct_definition.name
                ),
                name_mapping.name_to_property_name(&operation_definition_path, &parameter.name)
            )
        })
        .collect::<Vec<String>>()
        .join(",");
    if path_parameter_arguments.len() > 0 {
        path_parameter_arguments += ","
    }

    WebSocketRequestTemplate {
        module_imports: module_imports,
        enum_definitions: vec![],
        primitive_definitions: vec![],
        struct_definitions: struct_definitions
            .iter()
            .map(|&s| Into::<StructDefinitionTemplate>::into(s).serializable(false))
            .collect(),
        socket_stream_struct_name: format!(
            "{}Stream",
            name_mapping.name_to_struct_name(&operation_definition_path, &function_name)
        ),
        response_type_name: socket_transfer_type_definition.name.clone(),
        function_name: function_name.clone(),
        function_parameters: function_parameters,
        path_format_string: path_format_string,
        path_parameter_arguments: path_parameter_arguments,
        query_parameters_mutable: query_struct
            .properties
            .iter()
            .filter(|(_, property)| !property.required || property.type_name.starts_with("Vec<"))
            .collect::<Vec<(&String, &PropertyDefinition)>>()
            .len()
            > 0,
        query_parameters: query_struct
            .properties
            .iter()
            .map(|(_, property)| QueryParameter {
                real_name: property.real_name.clone(),
                name: property.name.clone(),
                struct_name: name_mapping
                    .name_to_property_name(&operation_definition_path, &query_struct.name),
                is_required: property.required,
                is_array: property.type_name.starts_with("Vec<"),
            })
            .collect(),
    }
    .render()
    .map_err(|err| err.to_string())
}

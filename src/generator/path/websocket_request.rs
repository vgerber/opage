use super::utils::{
    generate_request_body, generate_responses, is_path_parameter, use_module_to_string,
    TransferMediaType,
};
use crate::{
    generator::component::{
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
use log::error;
use oas3::{
    spec::{FromRef, ObjectOrReference, ObjectSchema, Operation, ParameterIn},
    Spec,
};
use std::collections::HashMap;

fn read_websocket_stream_to_string(struct_name: &str, response_type_name: &str) -> String {
    return format!(
        "pub struct {struct_name} {{
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    }}

impl {struct_name} {{
    pub fn from(socket: WebSocket<MaybeTlsStream<TcpStream>>) -> Self {{
        {struct_name} {{ socket: socket }}
    }}

    pub fn close(&mut self, code: Option<CloseFrame>) -> Result<(), Error> {{
        self.socket.close(code)
    }}

    pub fn read(&mut self) -> Result<{response_type_name}, String> {{
        let response = match self.socket.read() {{
            Ok(response) => response,
            Err(err) => return Err(err.to_string()),
        }};

        let response_text = match response.into_text() {{
            Ok(response) => response,
            Err(err) => return Err(err.to_string()),
        }};

        let result = match serde_json::from_str::<serde_json::Value>(&response_text) {{
            Ok(response_json_object) => response_json_object,
            Err(err) => return Err(err.to_string()),
        }};

        let response_object = match result.get(\"result\") {{
            Some(response_object) => response_object,
            None => return Err(\"No result in message\".to_string()),
        }};

        match serde_json::from_value::<{response_type_name}>(response_object.clone()) {{
            Ok(response_object) => Ok(response_object),
            Err(err) => return Err(err.to_string()),
        }}
    }}
}}
"
    );
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

    let mut request_source_code = String::new();

    let mut function_parameters = vec![];

    if !path_struct_definition.properties.is_empty() {
        function_parameters.push(format!(
            "{}: &{}",
            name_mapping
                .name_to_property_name(&operation_definition_path, &path_struct_definition.name),
            path_struct_definition.name
        ));
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
            path: "http".to_owned(),
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

    let mut query_struct_source_code = String::new();
    if query_struct.properties.len() > 0 {
        function_parameters.push(format!(
            "{}: &{}",
            name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
            query_struct.name
        ));
        query_struct_source_code += &query_struct.to_string(false);
        query_struct_source_code += "\n\n";
    }

    function_parameters.push("additional_headers: Option<Vec<(String, String)>>".to_owned());

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
                        function_parameters.push(format!(
                            "{}: {}",
                            name_mapping.name_to_property_name(
                                &operation_definition_path,
                                &type_definition.name
                            ),
                            type_definition.name
                        ))
                    }
                    None => (),
                },
                TransferMediaType::TextPlain => function_parameters.push(format!(
                    "request_string: &{}",
                    oas3_type_to_string(&oas3::spec::SchemaType::String)
                )),
            }
            break;
        }
    }

    let socket_stream_struct_name = format!(
        "{}Stream",
        name_mapping.name_to_struct_name(&operation_definition_path, &function_name)
    );

    request_source_code += &module_imports
        .iter()
        .map(use_module_to_string)
        .collect::<Vec<String>>()
        .join("\n");
    request_source_code += "\n\n";
    request_source_code += &read_websocket_stream_to_string(
        &socket_stream_struct_name,
        &socket_transfer_type_definition.name,
    );
    request_source_code += "\n";
    if !path_struct_definition.properties.is_empty() {
        request_source_code += &path_struct_definition.to_string(false);
        request_source_code += "\n";
    }

    request_source_code += &query_struct_source_code;

    // Function signature
    request_source_code += &format!(
        "pub async fn {}(host: &str, {}) -> Result<{}, tungstenite::Error> {{\n",
        function_name,
        function_parameters.join(", "),
        socket_stream_struct_name,
    );

    request_source_code += &format!(
        "let {} query_parameters: Vec<(&str, String)> = vec![{}];\n",
        match query_struct
            .properties
            .iter()
            .filter(|(_, property)| !property.required || property.type_name.starts_with("Vec<"))
            .collect::<Vec<(&String, &PropertyDefinition)>>()
            .len()
        {
            0 => "",
            _ => "mut",
        },
        query_struct
            .properties
            .iter()
            .filter(|(_, property)| property.required && !property.type_name.starts_with("Vec<"))
            .map(|(_, property)| format!(
                "(\"{}\",{}.{}.to_string())",
                property.real_name,
                name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
                property.name
            ))
            .collect::<Vec<String>>()
            .join(",")
    );

    query_struct
        .properties
        .values()
        .filter(|&property| property.required && property.type_name.starts_with("Vec<"))
        .for_each(|vector_property|
    {
        request_source_code += &format!(
                "{}.{}.iter().for_each(|query_parameter_item| query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
                name_mapping.name_to_property_name(&operation_definition_path, &vector_property.name),
                vector_property.real_name
            );
    });

    for optional_property in query_struct
        .properties
        .values()
        .filter(|&property| !property.required)
        .collect::<Vec<&PropertyDefinition>>()
    {
        request_source_code += &format!(
            "if let Some(ref query_parameter) = {}.{} {{\n",
            name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
            optional_property.name
        );
        if optional_property.type_name.starts_with("Vec<") {
            request_source_code += &format!(
                "query_parameter.iter().for_each(|query_parameter_item| query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                optional_property.real_name
            );
        } else {
            request_source_code += &format!(
                "query_parameters.push((\"{}\", query_parameter.to_string()));\n",
                optional_property.real_name
            );
        }
        request_source_code += "}\n"
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

    // create query parameter string
    request_source_code += "let mut query_string = query_parameters
        .iter()
        .map(|(name, value)| format!(\"{}={}\", name, value))
        .collect::<Vec<String>>()
        .join(\"&\");
    if query_string.len() > 0 {
        query_string.insert_str(0, \"?\");
    }";

    request_source_code += &format!(
        "let url = format!(
            \"{{}}{}{{}}\",
            host,
            {}
            query_string
        );",
        path_format_string, path_parameter_arguments
    );

    request_source_code += "
    let uri: Uri = match url.parse() {
        Ok(uri) => uri,
        Err(err) => return Err(err.into()),
    };

    let mut request = match uri.into_client_request() {
        Ok(request) => request,
        Err(err) => return Err(err),
    };
    let request_headers = request.headers_mut();

    if let Some(additional_headers) = additional_headers {
        additional_headers.iter().for_each(|(key, value)| {
            let key = match HeaderName::try_from(key) {
                Ok(key) => key,
                Err(_) => return (),
            };
            let value = match value.parse() {
                Ok(value) => value,
                Err(_) => return (),
            };
            request_headers.append(key, value);
        });
    }

    let (socket, _) = match connect(request) {
        Ok(connection) => connection,
        Err(err) => return Err(err),
    };";

    request_source_code += &format!("");
    request_source_code += &format!("Ok({}::from(socket))", socket_stream_struct_name);
    request_source_code += "}";
    Ok(request_source_code)
}

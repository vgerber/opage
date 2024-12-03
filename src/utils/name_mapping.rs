use convert_case::Casing;
use log::trace;
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct NameMapping {
    pub struct_mapping: HashMap<String, String>,
    pub property_mapping: HashMap<String, String>,
    pub module_mapping: HashMap<String, String>,
    pub status_code_mapping: HashMap<String, String>,
}

fn path_to_string(path: &Vec<String>, token_name: &str) -> String {
    let path_str = path.join("/");
    match path_str.len() {
        0 => format!("/{}", token_name),
        _ => format!("/{}/{}", path_str, token_name),
    }
    .replace("//", "/")
}

impl NameMapping {
    pub fn new() -> Self {
        NameMapping {
            module_mapping: HashMap::new(),
            property_mapping: HashMap::new(),
            struct_mapping: HashMap::new(),
            status_code_mapping: HashMap::new(),
        }
    }

    pub fn name_to_struct_name(&self, path: &Vec<String>, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Pascal);
        let path_str = path_to_string(path, &converted_name);

        trace!("name_to_struct_name {}", path_str);
        match self.struct_mapping.get(&path_str) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }

    pub fn name_to_property_name(&self, path: &Vec<String>, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Snake);
        let path_str = path_to_string(path, &converted_name);
        trace!("name_to_property_name {}", path_str);
        match self.property_mapping.get(&path_str) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }

    pub fn name_to_module_name(&self, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Snake);

        match self.module_mapping.get(&converted_name) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }

    pub fn status_code_to_canonical_name(&self, status_code: StatusCode) -> Result<String, String> {
        if let Some(canonical_name) = self.status_code_mapping.get(status_code.as_str()) {
            return Ok(canonical_name.clone());
        }

        match status_code.canonical_reason() {
            Some(canonical_status_code) => Ok(canonical_status_code.to_owned()),
            None => {
                return Err(format!(
                    "Failed to get canonical status code {}",
                    status_code
                ))
            }
        }
    }
}

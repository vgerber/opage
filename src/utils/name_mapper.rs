use convert_case::Casing;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, path::Path};

#[derive(Deserialize, Clone, Debug)]
struct NameMappings {
    pub struct_mapping: HashMap<String, String>,
    pub property_mapping: HashMap<String, String>,
    pub module_mapping: HashMap<String, String>,
}

pub struct NameMapper {
    mappings: NameMappings,
}

impl NameMapper {
    pub fn new(mapper_file_path: &Path) -> Result<Self, String> {
        let file = match File::open(mapper_file_path) {
            Ok(file) => file,
            Err(err) => return Err(err.to_string()),
        };
        let mappings: NameMappings = match serde_json::from_reader(file) {
            Ok(json_object) => json_object,
            Err(err) => return Err(err.to_string()),
        };
        Ok(NameMapper { mappings: mappings })
    }

    pub fn name_to_struct_name(&self, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Pascal);
        match self.mappings.struct_mapping.get(&converted_name) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }

    pub fn name_to_property_name(&self, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Snake);
        match self.mappings.property_mapping.get(&converted_name) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }

    pub fn name_to_module_name(&self, name: &str) -> String {
        let converted_name = name.to_case(convert_case::Case::Snake);
        match self.mappings.module_mapping.get(&converted_name) {
            Some(name) => name.clone(),
            None => converted_name,
        }
    }
}

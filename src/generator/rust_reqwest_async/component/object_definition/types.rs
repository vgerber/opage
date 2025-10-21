use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDefinition {
    pub name: String,
    pub module: Option<ModuleInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDefinition {
    pub name: String,
    pub real_name: String,
    pub type_name: String,
    pub module: Option<ModuleInfo>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectDefinition {
    Struct(StructDefinition),
    Enum(EnumDefinition),
    Primitive(PrimitveDefinition),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub value_type: TypeDefinition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDefinition {
    pub name: String,
    pub used_modules: Vec<ModuleInfo>,
    pub values: HashMap<String, EnumValue>,
}

pub type ObjectDatabase = HashMap<String, ObjectDefinition>;

impl EnumDefinition {
    pub fn get_required_modules(&self) -> Vec<&ModuleInfo> {
        let mut required_modules = self.used_modules.iter().collect::<Vec<&ModuleInfo>>();
        required_modules.append(
            &mut self
                .values
                .iter()
                .filter_map(|(_, enum_value)| enum_value.value_type.module.as_ref())
                .collect::<Vec<&ModuleInfo>>(),
        );
        required_modules
    }

    pub fn to_string(&self, serializable: bool) -> String {
        let mut definition_str = String::new();

        definition_str += match serializable {
            true => "#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]\n",
            _ => "",
        };
        definition_str += format!("pub enum {} {{\n\n", self.name).as_str();

        for (_, enum_value) in &self.values {
            definition_str +=
                format!("{}({}),\n", enum_value.name, enum_value.value_type.name).as_str()
        }

        definition_str += "}";
        definition_str
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDefinition {
    pub used_modules: Vec<ModuleInfo>,
    pub name: String,
    pub properties: HashMap<String, PropertyDefinition>,
    pub local_objects: HashMap<String, Box<ObjectDefinition>>,
}

impl StructDefinition {
    pub fn get_required_modules(&self) -> Vec<&ModuleInfo> {
        let mut required_modules = self.used_modules.iter().collect::<Vec<&ModuleInfo>>();
        required_modules.append(
            &mut self
                .properties
                .iter()
                .filter_map(|(_, property)| property.module.as_ref())
                .collect::<Vec<&ModuleInfo>>(),
        );
        required_modules
    }

    pub fn to_string(&self, serializable: bool) -> String {
        let mut definition_str = String::new();

        definition_str += match serializable {
            true => "#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]\n",
            _ => "",
        };
        definition_str += format!("pub struct {} {{\n\n", self.name).as_str();

        for (_, property) in &self.properties {
            if property.name != property.real_name && serializable {
                definition_str +=
                    format!("#[serde(alias = \"{}\")]\n", property.real_name).as_str();
            }

            match property.required {
                true => {
                    definition_str +=
                        format!("pub {}: {},\n", property.name, property.type_name).as_str()
                }
                false => {
                    definition_str +=
                        format!("pub {}: Option<{}>,\n", property.name, property.type_name).as_str()
                }
            }
        }

        definition_str += "}";
        definition_str
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimitveDefinition {
    pub name: String,
    pub primitive_type: TypeDefinition,
}

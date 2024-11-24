use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpecIgnore {
    paths: Vec<String>,
    components: Vec<String>,
}

impl SpecIgnore {
    pub fn new() -> Self {
        SpecIgnore {
            paths: vec![],
            components: vec![],
        }
    }

    pub fn path_ignored(&self, path: &str) -> bool {
        self.paths.contains(&path.to_owned())
    }

    pub fn component_ignored(&self, component: &str) -> bool {
        self.components.contains(&component.to_owned())
    }
}

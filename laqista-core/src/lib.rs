#![feature(async_closure)]
#![feature(async_fn_traits)]

pub mod client;
pub mod proto;
pub mod server;
pub mod session;
pub mod tensor;
pub mod wasm;

use std::collections::HashMap;

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DeploymentInfo {
    pub id: Uuid,
    pub name: String,
    pub source: String,
    pub accuracies: HashMap<String, f32>,
}

impl DeploymentInfo {
    pub fn new(name: String, source: String) -> Self {
        let id = Uuid::new_v4();
        let accuracies = HashMap::new();
        Self {
            name,
            source,
            id,
            accuracies,
        }
    }
}

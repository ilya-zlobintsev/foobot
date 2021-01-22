use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct DBConfig {
    pub host: String,
    pub port: u64,
    pub user: String,
    pub password: String,
    pub db: String,
}

impl DBConfig {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Ok(serde_json::from_str(json)?)
    }
}
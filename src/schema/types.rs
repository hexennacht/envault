use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SchemaType {
    #[default]
    String,
    Port,
    Url,
    Bool,
    Integer,
    Enum(Vec<String>),
}

impl SchemaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SchemaType::String => "string",
            SchemaType::Port => "port",
            SchemaType::Url => "url",
            SchemaType::Bool => "bool",
            SchemaType::Integer => "integer",
            SchemaType::Enum(_) => "enum",
        }
    }
}

impl std::fmt::Display for SchemaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaType::String => write!(f, "string"),
            SchemaType::Port => write!(f, "port"),
            SchemaType::Url => write!(f, "url"),
            SchemaType::Bool => write!(f, "bool"),
            SchemaType::Integer => write!(f, "integer"),
            SchemaType::Enum(variants) => write!(f, "enum({})", variants.join(",")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub key: String,
    pub required: bool,
    pub sensitive: bool,
    pub description: Option<String>,
    pub schema_type: SchemaType,
    pub default_value: Option<String>,
}

impl SchemaEntry {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            required: false,
            sensitive: false,
            description: None,
            schema_type: SchemaType::String,
            default_value: None,
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClientTheme {
    pub name: String,
    pub colors: ClientThemeColors,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct ClientThemeColors {
    pub background: Option<String>,
    pub text: Option<String>,
    pub accent: Option<String>,
    pub muted: Option<String>,
    pub border: Option<String>,
    pub surface: Option<String>,
    pub selection: Option<String>,
    pub working: Option<String>,
    pub blocked: Option<String>,
    pub done: Option<String>,
    pub unknown: Option<String>,
}

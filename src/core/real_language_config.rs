#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealLanguageConfig {
    pub provider_name: String,
    pub model_name: String,
    pub endpoint: String,
    pub timeout_ms: u64,
}

impl RealLanguageConfig {
    pub fn new_placeholder() -> Self {
        Self {
            provider_name: "placeholder".to_string(),
            model_name: "real-language-placeholder".to_string(),
            endpoint: "not-configured".to_string(),
            timeout_ms: 30_000,
        }
    }
}

impl Default for RealLanguageConfig {
    fn default() -> Self {
        Self::new_placeholder()
    }
}

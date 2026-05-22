#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealReasoningConfig {
    pub provider_name: String,
    pub model_name: String,
    pub endpoint: String,
    pub timeout_ms: u64,
}

impl RealReasoningConfig {
    pub fn new_placeholder() -> Self {
        Self {
            provider_name: "ollama".to_string(),
            model_name: "lens-logic:v0".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            timeout_ms: 90_000,
        }
    }
}

impl Default for RealReasoningConfig {
    fn default() -> Self {
        Self::new_placeholder()
    }
}

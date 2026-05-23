#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealLanguageConfig {
    pub provider_name: String,
    pub model_name: String,
    pub endpoint: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealLanguageConfigDiagnostics {
    pub is_complete: bool,
    pub issues: Vec<String>,
}

impl RealLanguageConfig {
    pub fn new_placeholder() -> Self {
        Self {
            provider_name: "ollama".to_string(),
            model_name: "qwen3:4b".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            timeout_ms: 300_000,
        }
    }

    pub fn diagnostics(&self) -> RealLanguageConfigDiagnostics {
        let mut issues = Vec::new();

        if self.provider_name.trim().is_empty() || self.provider_name == "placeholder" {
            issues.push("missing field: provider_name".to_string());
        }

        if self.model_name.trim().is_empty() || self.model_name == "real-language-placeholder" {
            issues.push("missing field: model".to_string());
        }

        issues.extend(endpoint_diagnostic_issues(self.endpoint.as_str()));

        if self.timeout_ms == 0 {
            issues.push("invalid field: timeout must be greater than 0".to_string());
        }

        RealLanguageConfigDiagnostics {
            is_complete: issues.is_empty(),
            issues,
        }
    }

    pub fn incomplete_reason(&self) -> Option<String> {
        let diagnostics = self.diagnostics();
        if diagnostics.is_complete {
            None
        } else {
            Some(diagnostics.issues.join("; "))
        }
    }
}

impl Default for RealLanguageConfig {
    fn default() -> Self {
        Self::new_placeholder()
    }
}

fn endpoint_diagnostic_issues(endpoint: &str) -> Vec<String> {
    let endpoint = endpoint.trim();

    if endpoint.is_empty() || endpoint == "not-configured" {
        return vec!["missing field: endpoint".to_string()];
    }

    let endpoint = endpoint.trim_end_matches('/');
    let Some(authority) = endpoint.strip_prefix("http://") else {
        return vec!["invalid field: endpoint must use local http base URL".to_string()];
    };

    if authority.contains('/') {
        return vec!["invalid field: endpoint must be a base URL without path".to_string()];
    }

    let Some((host, port_text)) = authority.rsplit_once(':') else {
        return vec!["invalid field: endpoint must include port".to_string()];
    };

    let host = host.trim();
    if host.is_empty() {
        return vec!["invalid field: endpoint host is empty".to_string()];
    }

    if host != "localhost" && host != "127.0.0.1" {
        return vec!["invalid field: endpoint must be local".to_string()];
    }

    if port_text.parse::<u16>().is_err() {
        return vec!["invalid field: endpoint port is invalid".to_string()];
    }

    Vec::new()
}

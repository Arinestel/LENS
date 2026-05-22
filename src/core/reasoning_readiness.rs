use crate::core::engine_selection::ReasoningEngineKind;
use crate::core::real_reasoning_config::RealReasoningConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasoningReadinessStatus {
    Ready,
    ConfigIncomplete { reason: String },
}

impl ReasoningReadinessStatus {
    pub fn log_message(&self) -> String {
        match self {
            Self::Ready => "reasoning readiness ready".to_string(),
            Self::ConfigIncomplete { reason } => {
                format!("reasoning readiness config incomplete: {reason}")
            }
        }
    }
}

pub struct ReasoningReadiness;

impl ReasoningReadiness {
    pub fn check(
        engine_kind: ReasoningEngineKind,
        real_config: &RealReasoningConfig,
    ) -> ReasoningReadinessStatus {
        match engine_kind {
            ReasoningEngineKind::Mock => ReasoningReadinessStatus::Ready,
            ReasoningEngineKind::Real => Self::check_real_config(real_config),
        }
    }

    fn check_real_config(real_config: &RealReasoningConfig) -> ReasoningReadinessStatus {
        if real_config.provider_name.trim().is_empty() {
            return Self::config_incomplete("provider_name is empty");
        }

        if real_config.model_name.trim().is_empty() {
            return Self::config_incomplete("model_name is empty");
        }

        if real_config.endpoint.trim().is_empty() || real_config.endpoint == "not-configured" {
            return Self::config_incomplete("endpoint is not configured");
        }

        if let Err(reason) = validate_local_http_base_url(&real_config.endpoint) {
            return Self::config_incomplete(reason.as_str());
        }

        if real_config.timeout_ms == 0 {
            return Self::config_incomplete("timeout_ms is zero");
        }

        if real_config.provider_name == "placeholder"
            || real_config.model_name == "real-reasoning-placeholder"
        {
            return Self::config_incomplete("real reasoning placeholder config is incomplete");
        }

        ReasoningReadinessStatus::Ready
    }

    fn config_incomplete(reason: &str) -> ReasoningReadinessStatus {
        ReasoningReadinessStatus::ConfigIncomplete {
            reason: reason.to_string(),
        }
    }
}

fn validate_local_http_base_url(endpoint: &str) -> Result<(), String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let authority = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| "endpoint must use local http base URL".to_string())?;

    if authority.contains('/') {
        return Err("endpoint must be a base URL without path".to_string());
    }

    let (host, port_text) = authority
        .rsplit_once(':')
        .ok_or_else(|| "endpoint must include port".to_string())?;

    if host != "localhost" && host != "127.0.0.1" {
        return Err("endpoint must be local".to_string());
    }

    port_text
        .parse::<u16>()
        .map(|_| ())
        .map_err(|_| "endpoint port is invalid".to_string())
}

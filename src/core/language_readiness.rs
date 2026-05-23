use crate::core::real_language_config::RealLanguageConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageReadinessStatus {
    Ready,
    NotConfigured { reason: String },
    Unavailable { reason: String },
}

pub struct LanguageReadiness;

impl LanguageReadiness {
    pub fn check_preview_config<F>(
        real_config: &RealLanguageConfig,
        check_model_available: F,
    ) -> LanguageReadinessStatus
    where
        F: FnOnce(&str) -> Result<bool, String>,
    {
        if let Err(reason) = Self::check_real_config(real_config) {
            return Self::not_configured(reason.as_str());
        }

        match check_model_available(real_config.model_name.trim()) {
            Ok(true) => LanguageReadinessStatus::Ready,
            Ok(false) => Self::unavailable("configured language model is not available"),
            Err(reason) => Self::unavailable(reason.as_str()),
        }
    }

    fn check_real_config(real_config: &RealLanguageConfig) -> Result<(), String> {
        real_config.incomplete_reason().map_or(Ok(()), Err)
    }

    fn not_configured(reason: &str) -> LanguageReadinessStatus {
        LanguageReadinessStatus::NotConfigured {
            reason: reason.to_string(),
        }
    }

    fn unavailable(reason: &str) -> LanguageReadinessStatus {
        LanguageReadinessStatus::Unavailable {
            reason: reason.to_string(),
        }
    }
}

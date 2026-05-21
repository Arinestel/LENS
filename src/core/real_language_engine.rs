use crate::core::language_engine::LanguageEngine;
use crate::core::orchestrator::CorePipelineError;
use crate::core::real_language_config::RealLanguageConfig;
use crate::core::reasoning_contract::ReasoningResult;

#[derive(Debug, Clone, PartialEq)]
pub struct RealLanguageEngine {
    config: RealLanguageConfig,
}

impl RealLanguageEngine {
    pub fn new(config: RealLanguageConfig) -> Self {
        Self { config }
    }
}

impl LanguageEngine for RealLanguageEngine {
    fn format_response(
        &self,
        _reasoning_result: &ReasoningResult,
    ) -> Result<String, CorePipelineError> {
        let _config = &self.config;

        Err(CorePipelineError {
            message: "Real language engine is not implemented yet.".to_string(),
        })
    }
}

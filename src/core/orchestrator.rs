// Цей файл зарезервовано для майбутнього оркестратора model integration layer.
// Тут визначено мінімальні контракти входу/виходу та mock orchestrator pipeline.
// Поки що тут не реалізовано реальну orchestration logic, validation або виклики моделей.

use crate::core::core_logger::CoreLogger;
use crate::core::core_input_context::CoreInputEnvelope;
use crate::core::core_runtime_config::CoreRuntimeConfig;
use crate::core::engine_selection::CoreEngineSelection;
use crate::core::language_engine::LanguageEngine;
use crate::core::reasoning_contract::ReasoningResult;
use crate::core::reasoning_engine::ReasoningEngine;
use crate::core::reasoning_validator::ReasoningValidator;

pub use crate::core::core_input_context::UserQuery;

#[derive(Debug, Clone, PartialEq)]
pub struct CoreOrchestrator;

#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratorRequest {
    pub input: CoreInputEnvelope,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratorResponse {
    pub user_facing_text: String,
    pub reasoning_result: Option<ReasoningResult>,
    pub confidence: f32,
    pub error: Option<CorePipelineError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorePipelineError {
    pub message: String,
}

impl From<UserQuery> for OrchestratorRequest {
    fn from(query: UserQuery) -> Self {
        Self {
            input: CoreInputEnvelope::from(query),
        }
    }
}

impl From<CoreInputEnvelope> for OrchestratorRequest {
    fn from(input: CoreInputEnvelope) -> Self {
        Self { input }
    }
}

impl CoreOrchestrator {
    pub fn run_mock_pipeline(request: OrchestratorRequest) -> OrchestratorResponse {
        let runtime_config = CoreRuntimeConfig::default();
        let engines = CoreEngineSelection::from_config(runtime_config).select_engines();

        Self::run_with_engines(request, engines.reasoning.as_ref(), engines.language.as_ref())
    }

    fn run_with_engines(
        request: OrchestratorRequest,
        reasoning_engine: &dyn ReasoningEngine,
        language_engine: &dyn LanguageEngine,
    ) -> OrchestratorResponse {
        CoreLogger::log("orchestrator", "mock pipeline started");

        let reasoning_result = match reasoning_engine.reason(&request) {
            Ok(reasoning_result) => reasoning_result,
            Err(error) => {
                CoreLogger::log("orchestrator", "mock pipeline failed");
                return OrchestratorResponse {
                    user_facing_text: String::new(),
                    reasoning_result: None,
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };

        if let Err(error) = ReasoningValidator::validate(&reasoning_result) {
            CoreLogger::log("orchestrator", "mock pipeline failed");
            return OrchestratorResponse {
                user_facing_text: String::new(),
                reasoning_result: Some(reasoning_result),
                confidence: 0.0,
                error: Some(error),
            };
        }

        let user_facing_text = match language_engine.format_response(&reasoning_result) {
            Ok(user_facing_text) => user_facing_text,
            Err(error) => {
                CoreLogger::log("orchestrator", "mock pipeline failed");
                return OrchestratorResponse {
                    user_facing_text: String::new(),
                    reasoning_result: Some(reasoning_result),
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };
        let confidence = reasoning_result.confidence;

        CoreLogger::log("orchestrator", "mock pipeline completed");

        OrchestratorResponse {
            user_facing_text,
            reasoning_result: Some(reasoning_result),
            confidence,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_pipeline_updates_core_log_file() {
        let before = std::fs::read_to_string(CoreLogger::log_path()).unwrap_or_default();

        let response = CoreOrchestrator::run_mock_pipeline(OrchestratorRequest::from(UserQuery {
            text: "test request".to_string(),
            language: "uk".to_string(),
            session_id: None,
        }));

        let after = std::fs::read_to_string(CoreLogger::log_path()).unwrap_or_default();

        assert!(response.error.is_none());
        assert!(after.len() > before.len());
        assert!(after.contains("[orchestrator] mock pipeline started"));
        assert!(after.contains("[logic_core] building mock reasoning result"));
        assert!(after.contains("[validator] validation passed"));
        assert!(after.contains("[language_layer] mock response formatted"));
        assert!(after.contains("[orchestrator] mock pipeline completed"));
    }
}

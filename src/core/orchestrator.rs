// Цей файл зарезервовано для майбутнього оркестратора model integration layer.
// Тут визначено мінімальні контракти входу/виходу та mock orchestrator pipeline.
// Поки що тут не реалізовано реальну orchestration logic, validation або виклики моделей.

use crate::core::core_input_context::CoreInputEnvelope;
use crate::core::core_logger::CoreLogger;
use crate::core::core_runtime_config::CoreRuntimeConfig;
use crate::core::engine_selection::CoreEngineSelection;
use crate::core::language_engine::LanguageEngine;
use crate::core::language_layer::LanguageLayer;
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::real_reasoning_engine::RealReasoningEngine;
use crate::core::reasoning_contract::ReasoningResult;
use crate::core::reasoning_engine::ReasoningEngine;
use crate::core::reasoning_readiness::{ReasoningReadiness, ReasoningReadinessStatus};
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
        let readiness = Self::check_reasoning_readiness(&runtime_config);
        CoreLogger::log("reasoning_readiness", readiness.log_message().as_str());

        let engines = CoreEngineSelection::from_config(runtime_config).select_engines();

        Self::run_with_engines(
            request,
            engines.reasoning.as_ref(),
            engines.language.as_ref(),
        )
    }

    pub fn run_manual_real_reasoning_test(
        request: OrchestratorRequest,
        real_config: RealReasoningConfig,
    ) -> OrchestratorResponse {
        let runtime_config = CoreRuntimeConfig::new_manual_real_reasoning_test();
        CoreLogger::log(
            "full_pipeline_test",
            "full_pipeline_test_started: source=UI; orchestrator=CoreOrchestrator; reasoning_source=Real; reasoning_backend=Ollama; language_source=Mock",
        );
        let readiness = ReasoningReadiness::check(runtime_config.reasoning_engine, &real_config);

        CoreLogger::log("manual_real_reasoning", readiness.log_message().as_str());

        if matches!(readiness, ReasoningReadinessStatus::ConfigIncomplete { .. }) {
            CoreLogger::log(
                "full_pipeline_test",
                "reasoning_error: stage=readiness_check; status=error; reasoning_source=Real; reasoning_backend=Ollama; message=reasoning config incomplete",
            );
            return OrchestratorResponse {
                user_facing_text: Self::format_full_pipeline_error(
                    "pipeline error",
                    readiness.log_message(),
                ),
                reasoning_result: None,
                confidence: 0.0,
                error: Some(CorePipelineError {
                    message: readiness.log_message(),
                }),
            };
        }

        let reasoning_engine = RealReasoningEngine::new(real_config);

        Self::run_manual_real_reasoning_with_engine(request, &reasoning_engine)
    }

    pub fn run_manual_raw_reasoning_result(
        request: OrchestratorRequest,
        real_config: RealReasoningConfig,
    ) -> OrchestratorResponse {
        let runtime_config = CoreRuntimeConfig::new_manual_real_reasoning_test();
        let readiness = ReasoningReadiness::check(runtime_config.reasoning_engine, &real_config);

        CoreLogger::log("raw_reasoning_result", readiness.log_message().as_str());

        if matches!(readiness, ReasoningReadinessStatus::ConfigIncomplete { .. }) {
            CoreLogger::log("raw_reasoning_result", "raw reasoning result path failed");
            return OrchestratorResponse {
                user_facing_text: String::new(),
                reasoning_result: None,
                confidence: 0.0,
                error: Some(CorePipelineError {
                    message: readiness.log_message(),
                }),
            };
        }

        let reasoning_engine = RealReasoningEngine::new(real_config);

        Self::run_manual_raw_reasoning_with_engine(request, &reasoning_engine)
    }

    pub fn check_reasoning_readiness(
        runtime_config: &CoreRuntimeConfig,
    ) -> ReasoningReadinessStatus {
        ReasoningReadiness::check(
            runtime_config.reasoning_engine,
            &RealReasoningConfig::default(),
        )
    }

    fn run_with_engines(
        request: OrchestratorRequest,
        reasoning_engine: &dyn ReasoningEngine,
        language_engine: &dyn LanguageEngine,
    ) -> OrchestratorResponse {
        Self::run_with_engines_labeled(request, reasoning_engine, language_engine, "mock pipeline")
    }

    fn run_with_engines_labeled(
        request: OrchestratorRequest,
        reasoning_engine: &dyn ReasoningEngine,
        language_engine: &dyn LanguageEngine,
        pipeline_label: &str,
    ) -> OrchestratorResponse {
        CoreLogger::log("orchestrator", format!("{pipeline_label} started").as_str());

        let reasoning_result = match reasoning_engine.reason(&request) {
            Ok(reasoning_result) => reasoning_result,
            Err(error) => {
                CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
                return OrchestratorResponse {
                    user_facing_text: String::new(),
                    reasoning_result: None,
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };

        if let Err(error) = ReasoningValidator::validate(&reasoning_result) {
            CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
            return OrchestratorResponse {
                user_facing_text: String::new(),
                reasoning_result: Some(reasoning_result),
                confidence: 0.0,
                error: Some(error),
            };
        }

        let user_facing_text = match language_engine.format_response(
            &reasoning_result,
            request.input.session_context.request_language.as_str(),
        ) {
            Ok(user_facing_text) => user_facing_text,
            Err(error) => {
                CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
                return OrchestratorResponse {
                    user_facing_text: String::new(),
                    reasoning_result: Some(reasoning_result),
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };
        let confidence = reasoning_result.confidence;

        CoreLogger::log(
            "orchestrator",
            format!("{pipeline_label} completed").as_str(),
        );

        OrchestratorResponse {
            user_facing_text,
            reasoning_result: Some(reasoning_result),
            confidence,
            error: None,
        }
    }

    fn run_manual_real_reasoning_with_engine(
        request: OrchestratorRequest,
        reasoning_engine: &dyn ReasoningEngine,
    ) -> OrchestratorResponse {
        let pipeline_label = "full pipeline test";
        CoreLogger::log("orchestrator", format!("{pipeline_label} started").as_str());
        CoreLogger::log(
            "full_pipeline_test",
            "reasoning_started: stage=reasoning; status=started; reasoning_source=Real; reasoning_backend=Ollama",
        );

        let reasoning_result = match reasoning_engine.reason(&request) {
            Ok(reasoning_result) => reasoning_result,
            Err(error) => {
                CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
                CoreLogger::log(
                    "full_pipeline_test",
                    format!(
                        "reasoning_error: stage=reasoning; status=error; reasoning_source=Real; reasoning_backend=Ollama; message={}",
                        error.message
                    )
                    .as_str(),
                );
                return OrchestratorResponse {
                    user_facing_text: Self::format_full_pipeline_error(
                        "reasoning error",
                        &error.message,
                    ),
                    reasoning_result: None,
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };
        CoreLogger::log(
            "full_pipeline_test",
            "reasoning_completed: stage=reasoning; status=success; output=ReasoningResult",
        );

        if let Err(error) = ReasoningValidator::validate(&reasoning_result) {
            CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
            CoreLogger::log(
                "full_pipeline_test",
                format!(
                    "pipeline_error: stage=reasoning_validation; status=error; message={}",
                    error.message
                )
                .as_str(),
            );
            return OrchestratorResponse {
                user_facing_text: Self::format_full_pipeline_error(
                    "pipeline error",
                    &error.message,
                ),
                reasoning_result: Some(reasoning_result),
                confidence: 0.0,
                error: Some(error),
            };
        }

        CoreLogger::log(
            "full_pipeline_test",
            "language_formatting_started: stage=language_formatting; status=started; language_source=Mock; input=ReasoningResult",
        );
        let user_facing_text = LanguageLayer::format_manual_real_reasoning_response(
            &reasoning_result,
            request.input.session_context.request_language.as_str(),
        );
        CoreLogger::log(
            "full_pipeline_test",
            "language_formatting_completed: stage=language_formatting; status=success; language_source=Mock; output=user_facing_answer",
        );
        let confidence = reasoning_result.confidence;

        CoreLogger::log(
            "orchestrator",
            format!("{pipeline_label} completed").as_str(),
        );
        CoreLogger::log(
            "full_pipeline_test",
            "full_pipeline_test_completed: status=success; output=user_facing_answer; reasoning_source=Real; reasoning_backend=Ollama; language_source=Mock",
        );

        OrchestratorResponse {
            user_facing_text,
            reasoning_result: Some(reasoning_result),
            confidence,
            error: None,
        }
    }

    fn run_manual_raw_reasoning_with_engine(
        request: OrchestratorRequest,
        reasoning_engine: &dyn ReasoningEngine,
    ) -> OrchestratorResponse {
        let pipeline_label = "raw reasoning result path";
        CoreLogger::log("orchestrator", format!("{pipeline_label} started").as_str());

        let reasoning_result = match reasoning_engine.reason(&request) {
            Ok(reasoning_result) => reasoning_result,
            Err(error) => {
                CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
                return OrchestratorResponse {
                    user_facing_text: String::new(),
                    reasoning_result: None,
                    confidence: 0.0,
                    error: Some(error),
                };
            }
        };

        if let Err(error) = ReasoningValidator::validate(&reasoning_result) {
            CoreLogger::log("orchestrator", format!("{pipeline_label} failed").as_str());
            return OrchestratorResponse {
                user_facing_text: String::new(),
                reasoning_result: Some(reasoning_result),
                confidence: 0.0,
                error: Some(error),
            };
        }

        let confidence = reasoning_result.confidence;

        CoreLogger::log(
            "orchestrator",
            format!("{pipeline_label} completed").as_str(),
        );

        OrchestratorResponse {
            user_facing_text: String::new(),
            reasoning_result: Some(reasoning_result),
            confidence,
            error: None,
        }
    }

    fn format_full_pipeline_error(stage: &str, error: impl AsRef<str>) -> String {
        format!(
            "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\nfull pipeline test {stage}: {}",
            error.as_ref()
        )
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

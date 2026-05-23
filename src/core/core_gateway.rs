use crate::core::core_input_context::{CoreInputEnvelope, CoreSessionContext, UserQuery};
use crate::core::core_logger::CoreLogger;
use crate::core::language_layer::LanguageLayer;
use crate::core::orchestrator::{CoreOrchestrator, OrchestratorRequest};
use crate::core::real_language_config::RealLanguageConfig;
use crate::core::real_language_engine::{RealLanguageAdapterRequest, RealLanguageEngine};
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::reasoning_contract::ReasoningResult;

#[derive(Debug, Clone, PartialEq)]
pub struct CoreGateway;

#[derive(Debug, Clone, PartialEq)]
pub struct UiCoreRequest {
    pub text: String,
    pub language: String,
    pub session_id: Option<String>,
    pub branch_id: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiCoreResponse {
    pub response_text: String,
    pub reasoning_result: Option<ReasoningResult>,
    pub confidence: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiRawReasoningResponse {
    pub reasoning_result: Option<ReasoningResult>,
    pub confidence: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiRealLanguageRequestPreview {
    pub request: Option<RealLanguageAdapterRequest>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiRealLanguagePreviewTestResponse {
    pub generated_text: String,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiLanguageComparisonResponse {
    pub mock_output: String,
    pub real_output: Option<String>,
    pub real_warnings: Vec<String>,
    pub real_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiFullPipelineRealLanguagePreviewResponse {
    pub reasoning_result: Option<ReasoningResult>,
    pub generated_text: String,
    pub warnings: Vec<String>,
    pub error_stage: Option<String>,
    pub error: Option<String>,
}

const INTERNAL_REASONING_LEAKAGE_WARNING: &str =
    "warning: language output appears to include internal reasoning or pre-answer analysis";
const REQUIRED_LANGUAGE_TEMPLATE_HEADER: &str = "Краткий вывод:";
const REQUIRED_LANGUAGE_TEMPLATE_HEADER_WARNING: &str =
    "warning: language output does not start with required template header";
const THINK_TRACE_END_MARKER: &str = "</think>";
const PRE_ANSWER_TRACE_REMOVED_NOTE: &str =
    "note: pre-answer trace before </think> was removed from preview output";

impl CoreGateway {
    pub fn run_mock_pipeline(request: UiCoreRequest) -> UiCoreResponse {
        Self::run_with_orchestrator_response(CoreOrchestrator::run_mock_pipeline(
            OrchestratorRequest::from(Self::build_input(request)),
        ))
    }

    pub fn run_manual_real_reasoning_test(
        request: UiCoreRequest,
        real_config: RealReasoningConfig,
    ) -> UiCoreResponse {
        Self::run_with_orchestrator_response(CoreOrchestrator::run_manual_real_reasoning_test(
            OrchestratorRequest::from(Self::build_input(request)),
            real_config,
        ))
    }

    pub fn run_manual_raw_reasoning_result(
        request: UiCoreRequest,
        real_config: RealReasoningConfig,
    ) -> UiRawReasoningResponse {
        let response = CoreOrchestrator::run_manual_raw_reasoning_result(
            OrchestratorRequest::from(Self::build_input(request)),
            real_config,
        );

        UiRawReasoningResponse {
            reasoning_result: response.reasoning_result,
            confidence: response.confidence,
            error: response.error.map(|error| error.message),
        }
    }

    pub fn preview_real_language_request(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
        real_config: RealLanguageConfig,
    ) -> UiRealLanguageRequestPreview {
        let engine = RealLanguageEngine::new(real_config);
        match engine.prepare_request(reasoning_result, requested_language) {
            Ok(request) => UiRealLanguageRequestPreview {
                request: Some(request),
                error: None,
            },
            Err(error) => UiRealLanguageRequestPreview {
                request: None,
                error: Some(error.message),
            },
        }
    }

    pub fn run_real_language_preview_test(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
        real_config: RealLanguageConfig,
    ) -> UiRealLanguagePreviewTestResponse {
        let engine = RealLanguageEngine::new(real_config);
        match engine.generate_preview(reasoning_result, requested_language) {
            Ok(generated_text) => {
                match Self::extract_final_answer_after_think_marker(generated_text.as_str()) {
                    Ok((generated_text, mut warnings)) => {
                        warnings
                            .extend(Self::validate_real_language_preview_output(&generated_text));

                        UiRealLanguagePreviewTestResponse {
                            warnings,
                            generated_text,
                            error: None,
                        }
                    }
                    Err(error) => UiRealLanguagePreviewTestResponse {
                        generated_text: String::new(),
                        warnings: Vec::new(),
                        error: Some(error),
                    },
                }
            }
            Err(error) => UiRealLanguagePreviewTestResponse {
                generated_text: String::new(),
                warnings: Vec::new(),
                error: Some(error.message),
            },
        }
    }

    pub fn compare_mock_vs_real_language_output(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
        real_config: RealLanguageConfig,
    ) -> UiLanguageComparisonResponse {
        let mock_output = LanguageLayer::format_manual_real_reasoning_response(
            reasoning_result,
            requested_language,
        );
        let real_preview =
            Self::run_real_language_preview_test(reasoning_result, requested_language, real_config);

        UiLanguageComparisonResponse {
            mock_output,
            real_output: if real_preview.error.is_none() {
                Some(real_preview.generated_text)
            } else {
                None
            },
            real_warnings: real_preview.warnings,
            real_error: real_preview.error,
        }
    }

    pub fn run_full_pipeline_with_real_language_preview(
        request: UiCoreRequest,
        reasoning_config: RealReasoningConfig,
        real_config: RealLanguageConfig,
    ) -> UiFullPipelineRealLanguagePreviewResponse {
        CoreLogger::log(
            "full_pipeline_real_language_preview",
            "full_pipeline_with_real_language_preview_started: source=UI; reasoning_source=Real; reasoning_backend=Ollama; language_source=RealPreview",
        );

        let requested_language = request.language.clone();
        let reasoning_response = CoreOrchestrator::run_manual_raw_reasoning_result(
            OrchestratorRequest::from(Self::build_input(request)),
            reasoning_config,
        );

        if let Some(error) = reasoning_response.error {
            CoreLogger::log(
                "full_pipeline_real_language_preview",
                format!(
                    "reasoning_error: stage=reasoning; status=error; message={}",
                    error.message
                )
                .as_str(),
            );
            return UiFullPipelineRealLanguagePreviewResponse {
                reasoning_result: reasoning_response.reasoning_result,
                generated_text: String::new(),
                warnings: Vec::new(),
                error_stage: Some("reasoning".to_string()),
                error: Some(error.message),
            };
        }

        let Some(reasoning_result) = reasoning_response.reasoning_result else {
            CoreLogger::log(
                "full_pipeline_real_language_preview",
                "reasoning_error: stage=reasoning; status=error; message=empty ReasoningResult",
            );
            return UiFullPipelineRealLanguagePreviewResponse {
                reasoning_result: None,
                generated_text: String::new(),
                warnings: Vec::new(),
                error_stage: Some("reasoning".to_string()),
                error: Some("empty ReasoningResult".to_string()),
            };
        };

        CoreLogger::log(
            "full_pipeline_real_language_preview",
            "reasoning_completed: stage=reasoning; status=success; output=ReasoningResult",
        );

        let real_preview = Self::run_real_language_preview_test(
            &reasoning_result,
            requested_language.as_str(),
            real_config,
        );

        if let Some(error) = real_preview.error {
            CoreLogger::log(
                "full_pipeline_real_language_preview",
                format!(
                    "language_preview_error: stage=language_preview; status=error; message={error}"
                )
                .as_str(),
            );
            return UiFullPipelineRealLanguagePreviewResponse {
                reasoning_result: Some(reasoning_result),
                generated_text: String::new(),
                warnings: Vec::new(),
                error_stage: Some("language preview".to_string()),
                error: Some(error),
            };
        }

        CoreLogger::log(
            "full_pipeline_real_language_preview",
            "language_preview_completed: stage=language_preview; status=success; output=generated_preview_answer",
        );
        CoreLogger::log(
            "full_pipeline_real_language_preview",
            "full_pipeline_with_real_language_preview_completed: status=success; output=real_language_preview_answer",
        );

        UiFullPipelineRealLanguagePreviewResponse {
            reasoning_result: Some(reasoning_result),
            generated_text: real_preview.generated_text,
            warnings: real_preview.warnings,
            error_stage: None,
            error: None,
        }
    }

    fn validate_real_language_preview_output(generated_text: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        let trimmed = generated_text.trim();

        if generated_text.contains("```") {
            warnings.push("warning: language output contains code fences".to_string());
        }

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            warnings.push(
                "warning: language output looks like structured JSON, expected user-facing text"
                    .to_string(),
            );
        }

        if !trimmed.starts_with(REQUIRED_LANGUAGE_TEMPLATE_HEADER) {
            warnings.push(REQUIRED_LANGUAGE_TEMPLATE_HEADER_WARNING.to_string());
        }

        if Self::has_internal_reasoning_leakage(trimmed) {
            warnings.push(INTERNAL_REASONING_LEAKAGE_WARNING.to_string());
        }

        if trimmed.len() < 12 {
            warnings.push("warning: language output is very short".to_string());
        }

        warnings
    }

    fn extract_final_answer_after_think_marker(
        generated_text: &str,
    ) -> Result<(String, Vec<String>), String> {
        let Some((_, final_answer)) = generated_text.rsplit_once(THINK_TRACE_END_MARKER) else {
            return Ok((generated_text.to_string(), Vec::new()));
        };

        let final_answer = final_answer.trim();
        if final_answer.is_empty() {
            return Err("language preview final answer after </think> is empty".to_string());
        }

        Ok((
            final_answer.to_string(),
            vec![PRE_ANSWER_TRACE_REMOVED_NOTE.to_string()],
        ))
    }

    fn has_internal_reasoning_leakage(generated_text: &str) -> bool {
        let early_text = generated_text.lines().take(4).collect::<Vec<_>>().join(" ");
        let normalized = early_text
            .trim_start_matches(|character: char| {
                character.is_whitespace()
                    || character == '"'
                    || character == '\''
                    || character == '-'
                    || character == '*'
            })
            .to_lowercase();

        let start_markers = [
            "okay",
            "let's",
            "lets",
            "i need",
            "i need to",
            "first, i",
            "first,",
            "we are given",
            "the reasoningresult provides",
            "the reasoningresult says",
            "the reasoning result provides",
            "the reasoning result says",
            "the task is",
            "let me",
            "the user wants",
            "i should",
            "i will",
            "мне нужно",
            "начну",
            "хорошо",
            "итак",
            "сначала",
            "давай",
            "разбер",
            "пользователь просит",
        ];

        if start_markers
            .iter()
            .any(|marker| normalized.starts_with(marker))
        {
            return true;
        }

        let contains_markers = [
            "reasoning",
            "chain of thought",
            "шаблон",
            "инструкция",
            "предоставленные поля",
            "source fields",
            "input fields",
            "output template",
            "data:",
            "return exactly",
            "мне нужно",
            "начну",
        ];

        contains_markers
            .iter()
            .any(|marker| normalized.contains(marker))
    }

    fn build_input(request: UiCoreRequest) -> CoreInputEnvelope {
        let user_query = UserQuery {
            text: request.text,
            language: request.language.clone(),
            session_id: request.session_id.clone(),
        };

        let session_context = CoreSessionContext {
            session_id: request.session_id,
            request_language: request.language,
            branch_id: request.branch_id,
            user_id: request.user_id,
        };

        CoreInputEnvelope {
            query: user_query,
            session_context,
        }
    }

    fn run_with_orchestrator_response(
        response: crate::core::orchestrator::OrchestratorResponse,
    ) -> UiCoreResponse {
        UiCoreResponse {
            response_text: response.user_facing_text,
            reasoning_result: response.reasoning_result,
            confidence: response.confidence,
            error: response.error.map(|error| error.message),
        }
    }
}

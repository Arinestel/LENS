use crate::core::core_input_context::{CoreInputEnvelope, CoreSessionContext, UserQuery};
use crate::core::orchestrator::{CoreOrchestrator, OrchestratorRequest};
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
    pub confidence: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiRawReasoningResponse {
    pub reasoning_result: Option<ReasoningResult>,
    pub confidence: f32,
    pub error: Option<String>,
}

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
            confidence: response.confidence,
            error: response.error.map(|error| error.message),
        }
    }
}

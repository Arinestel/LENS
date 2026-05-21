use crate::core::core_input_context::{CoreInputEnvelope, CoreSessionContext, UserQuery};
use crate::core::orchestrator::{CoreOrchestrator, OrchestratorRequest};

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

impl CoreGateway {
    pub fn run_mock_pipeline(request: UiCoreRequest) -> UiCoreResponse {
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

        let input = CoreInputEnvelope {
            query: user_query,
            session_context,
        };

        let response = CoreOrchestrator::run_mock_pipeline(OrchestratorRequest::from(input));

        UiCoreResponse {
            response_text: response.user_facing_text,
            confidence: response.confidence,
            error: response.error.map(|error| error.message),
        }
    }
}

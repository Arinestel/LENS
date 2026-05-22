use crate::core::ollama_reasoning_client::OllamaReasoningClient;
use crate::core::orchestrator::{CorePipelineError, OrchestratorRequest};
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::reasoning_contract::ReasoningResult;
use crate::core::reasoning_engine::ReasoningEngine;
use crate::core::reasoning_response_mapping::map_raw_reasoning_backend_response;

#[derive(Debug, Clone, PartialEq)]
pub struct RealReasoningEngine {
    config: RealReasoningConfig,
}

impl RealReasoningEngine {
    pub fn new(config: RealReasoningConfig) -> Self {
        Self { config }
    }
}

impl ReasoningEngine for RealReasoningEngine {
    fn reason(&self, request: &OrchestratorRequest) -> Result<ReasoningResult, CorePipelineError> {
        let client = OllamaReasoningClient::new(self.config.clone());
        let raw_response = client.generate(request)?;

        map_raw_reasoning_backend_response(raw_response).map_err(CorePipelineError::from)
    }
}

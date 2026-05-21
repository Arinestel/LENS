use crate::core::orchestrator::{CorePipelineError, OrchestratorRequest};
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::reasoning_contract::ReasoningResult;
use crate::core::reasoning_engine::ReasoningEngine;

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
    fn reason(
        &self,
        _request: &OrchestratorRequest,
    ) -> Result<ReasoningResult, CorePipelineError> {
        let _config = &self.config;

        Err(CorePipelineError {
            message: "Real reasoning engine is not implemented yet.".to_string(),
        })
    }
}

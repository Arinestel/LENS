use crate::core::orchestrator::{CorePipelineError, OrchestratorRequest};
use crate::core::reasoning_contract::ReasoningResult;

pub trait ReasoningEngine {
    fn reason(
        &self,
        request: &OrchestratorRequest,
    ) -> Result<ReasoningResult, CorePipelineError>;
}

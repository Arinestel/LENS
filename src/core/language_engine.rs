use crate::core::orchestrator::CorePipelineError;
use crate::core::reasoning_contract::ReasoningResult;

pub trait LanguageEngine {
    fn format_response(
        &self,
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> Result<String, CorePipelineError>;
}

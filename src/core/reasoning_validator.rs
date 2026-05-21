// Цей файл відповідає за мінімальну валідацію ReasoningResult у mock core pipeline.
// Тут уже визначено вузький validator path для базового контракту reasoning layer.
// Поки що тут не реалізовано складну доменну логіку, зовнішні перевірки або model validation.

use crate::core::orchestrator::CorePipelineError;
use crate::core::core_logger::CoreLogger;
use crate::core::reasoning_contract::ReasoningResult;

#[derive(Debug, Clone, PartialEq)]
pub struct ReasoningValidator;

impl ReasoningValidator {
    pub fn validate(reasoning_result: &ReasoningResult) -> Result<(), CorePipelineError> {
        CoreLogger::log("validator", "validating mock reasoning result");

        if reasoning_result.task.trim().is_empty() {
            CoreLogger::log("validator", "validation failed: task is empty");
            return Err(CorePipelineError {
                message: "ReasoningResult task is empty.".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&reasoning_result.confidence) {
            CoreLogger::log("validator", "validation failed: confidence is out of range");
            return Err(CorePipelineError {
                message: "ReasoningResult confidence is out of range.".to_string(),
            });
        }

        CoreLogger::log("validator", "validation passed");

        Ok(())
    }
}

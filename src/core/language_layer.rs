// Цей файл зарезервовано для майбутнього language layer LENS App.
// Тут визначено мінімальний placeholder-тип мовного шару та mock formatter.
// Поки що тут не реалізовано реальну генерацію тексту, language formatting або виклики моделей.

use crate::core::core_logger::CoreLogger;
use crate::core::language_engine::LanguageEngine;
use crate::core::orchestrator::CorePipelineError;
use crate::core::reasoning_contract::ReasoningResult;

#[derive(Debug, Clone, PartialEq)]
pub struct LanguageLayer;

impl LanguageLayer {
    pub fn format_mock_response(reasoning_result: &ReasoningResult) -> String {
        CoreLogger::log("language_layer", "formatting mock response");

        let conclusion = reasoning_result
            .conclusions
            .first()
            .map(|item| item.text.as_str())
            .unwrap_or("Mock reasoning result без висновку.");

        let response = format!("[MOCK CORE] {} Task: {}", conclusion, reasoning_result.task);

        CoreLogger::log("language_layer", "mock response formatted");

        response
    }

    pub fn format_manual_real_reasoning_response(reasoning_result: &ReasoningResult) -> String {
        CoreLogger::log(
            "language_layer",
            "formatting manual real reasoning response with mock language layer",
        );

        let conclusion = reasoning_result
            .conclusions
            .first()
            .map(|item| item.text.as_str())
            .unwrap_or("Mock language formatting has no conclusion text.");

        let response = format!(
            "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\n{} Task: {}",
            conclusion, reasoning_result.task
        );

        CoreLogger::log(
            "language_layer",
            "manual real reasoning response formatted; reasoning source: Real; reasoning backend: Ollama; language source: Mock",
        );

        response
    }
}

impl LanguageEngine for LanguageLayer {
    fn format_response(
        &self,
        reasoning_result: &ReasoningResult,
    ) -> Result<String, CorePipelineError> {
        Ok(Self::format_mock_response(reasoning_result))
    }
}

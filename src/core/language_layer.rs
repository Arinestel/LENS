// Цей файл відповідає за mock Language Layer LENS App.
// Він перетворює вже готовий ReasoningResult у user-facing text без зміни reasoning logic.
// Тут не має бути викликів моделей, raw Ollama parsing або fallback через thinking.

use crate::core::core_logger::CoreLogger;
use crate::core::language_engine::LanguageEngine;
use crate::core::orchestrator::CorePipelineError;
use crate::core::reasoning_contract::ReasoningResult;

#[derive(Debug, Clone, PartialEq)]
pub struct LanguageLayer;

impl LanguageLayer {
    pub fn format_mock_response(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> String {
        CoreLogger::log("language_layer", "formatting mock response");

        let response =
            Self::format_user_facing_reasoning_answer(reasoning_result, requested_language);

        CoreLogger::log("language_layer", "mock response formatted");

        response
    }

    pub fn format_manual_real_reasoning_response(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> String {
        CoreLogger::log(
            "language_layer",
            "formatting manual real reasoning response with mock language layer",
        );

        let response = format!(
            "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\n{}",
            Self::format_user_facing_reasoning_answer(reasoning_result, requested_language)
        );

        CoreLogger::log(
            "language_layer",
            "manual real reasoning response formatted; reasoning source: Real; reasoning backend: Ollama; language source: Mock",
        );

        response
    }

    fn format_user_facing_reasoning_answer(
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> String {
        if requested_language.eq_ignore_ascii_case("uk") {
            return Self::format_ukrainian_reasoning_answer(reasoning_result);
        }

        Self::format_english_reasoning_answer(reasoning_result)
    }

    fn format_ukrainian_reasoning_answer(reasoning_result: &ReasoningResult) -> String {
        format!(
            "Короткий підсумок:\nЗадача: {}\nВисновок: {}\n\nФакти:\n{}\n\nВисновки:\n{}\n\nПрипущення:\n{}\n\nНевизначеності:\n{}\n\nНаступні дії:\n{}\n\nВпевненість: {:.2}",
            reasoning_result.task,
            Self::first_text_or_none(
                reasoning_result
                    .conclusions
                    .first()
                    .map(|item| item.text.as_str())
            ),
            Self::format_text_items(
                reasoning_result
                    .facts
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .conclusions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .assumptions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .uncertainties
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .next_actions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            reasoning_result.confidence,
        )
    }

    fn format_english_reasoning_answer(reasoning_result: &ReasoningResult) -> String {
        format!(
            "Short summary:\nTask: {}\nConclusion: {}\n\nFacts:\n{}\n\nConclusions:\n{}\n\nAssumptions:\n{}\n\nUncertainties:\n{}\n\nNext actions:\n{}\n\nConfidence: {:.2}",
            reasoning_result.task,
            Self::first_text_or_none(
                reasoning_result
                    .conclusions
                    .first()
                    .map(|item| item.text.as_str())
            ),
            Self::format_text_items(
                reasoning_result
                    .facts
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .conclusions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .assumptions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .uncertainties
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_text_items(
                reasoning_result
                    .next_actions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            reasoning_result.confidence,
        )
    }

    fn first_text_or_none(item: Option<&str>) -> &str {
        item.filter(|text| !text.trim().is_empty())
            .unwrap_or("none")
    }

    fn format_text_items(items: Vec<&str>) -> String {
        if items.is_empty() {
            return "- none".to_string();
        }

        let mut formatted = String::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                formatted.push('\n');
            }
            formatted.push_str("- ");
            formatted.push_str(item);
        }
        formatted
    }
}

impl LanguageEngine for LanguageLayer {
    fn format_response(
        &self,
        reasoning_result: &ReasoningResult,
        requested_language: &str,
    ) -> Result<String, CorePipelineError> {
        Ok(Self::format_mock_response(
            reasoning_result,
            requested_language,
        ))
    }
}

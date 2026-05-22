use crate::core::reasoning_contract::{
    AssumptionItem, ConclusionItem, FactItem, NextActionItem, ReasoningResult, UncertaintyItem,
};
use crate::core::reasoning_errors::ReasoningEngineError;

#[derive(Debug, Clone, PartialEq)]
pub struct RawReasoningBackendResponse {
    pub task: Option<String>,
    pub facts: Option<Vec<String>>,
    pub conclusions: Option<Vec<String>>,
    pub assumptions: Option<Vec<String>>,
    pub uncertainties: Option<Vec<String>>,
    pub next_actions: Option<Vec<String>>,
    pub confidence: Option<f32>,
}

impl RawReasoningBackendResponse {
    pub fn placeholder_incomplete() -> Self {
        Self {
            task: None,
            facts: None,
            conclusions: None,
            assumptions: None,
            uncertainties: None,
            next_actions: None,
            confidence: None,
        }
    }
}

pub fn map_raw_reasoning_backend_response(
    raw_response: RawReasoningBackendResponse,
) -> Result<ReasoningResult, ReasoningEngineError> {
    let task = required_text(raw_response.task, "task")?;
    let facts = required_items(raw_response.facts, "facts")?;
    let conclusions = required_items(raw_response.conclusions, "conclusions")?;
    let assumptions = required_items(raw_response.assumptions, "assumptions")?;
    let uncertainties = required_items(raw_response.uncertainties, "uncertainties")?;
    let next_actions = required_items(raw_response.next_actions, "next_actions")?;
    let confidence = raw_response
        .confidence
        .ok_or_else(|| mapping_error("confidence is missing"))?;

    if !(0.0..=1.0).contains(&confidence) {
        return Err(mapping_error("confidence is out of range"));
    }

    if facts.is_empty()
        && conclusions.is_empty()
        && assumptions.is_empty()
        && uncertainties.is_empty()
        && next_actions.is_empty()
    {
        return Err(mapping_error(
            "raw response has no mappable reasoning content",
        ));
    }

    Ok(ReasoningResult {
        task,
        facts: facts.into_iter().map(|text| FactItem { text }).collect(),
        conclusions: conclusions
            .into_iter()
            .map(|text| ConclusionItem { text })
            .collect(),
        assumptions: assumptions
            .into_iter()
            .map(|text| AssumptionItem { text })
            .collect(),
        uncertainties: uncertainties
            .into_iter()
            .map(|text| UncertaintyItem { text })
            .collect(),
        next_actions: next_actions
            .into_iter()
            .map(|text| NextActionItem { text })
            .collect(),
        confidence,
    })
}

fn required_text(value: Option<String>, field_name: &str) -> Result<String, ReasoningEngineError> {
    let text = value.ok_or_else(|| mapping_error(format!("{field_name} is missing").as_str()))?;
    let text = text.trim();

    if text.is_empty() {
        return Err(mapping_error(format!("{field_name} is empty").as_str()));
    }

    Ok(text.to_string())
}

fn required_items(
    value: Option<Vec<String>>,
    field_name: &str,
) -> Result<Vec<String>, ReasoningEngineError> {
    let items = value.ok_or_else(|| mapping_error(format!("{field_name} is missing").as_str()))?;
    let mut mapped_items = Vec::with_capacity(items.len());

    for item in items {
        let item = item.trim();

        if item.is_empty() {
            return Err(mapping_error(
                format!("{field_name} contains an empty item").as_str(),
            ));
        }

        mapped_items.push(item.to_string());
    }

    Ok(mapped_items)
}

fn mapping_error(message: &str) -> ReasoningEngineError {
    ReasoningEngineError::MappingError {
        message: message.to_string(),
    }
}

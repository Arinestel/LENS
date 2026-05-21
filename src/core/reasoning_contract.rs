// Цей файл визначає базовий контракт структурованого результату reasoning layer.
// Його роль у системі: зафіксувати прості типи даних для майбутньої інтеграції моделей.
// Поки що тут не реалізовано reasoning logic, validation, pipeline або методи обробки.

#[derive(Debug, Clone, PartialEq)]
pub struct ReasoningResult {
    pub task: String,
    pub facts: Vec<FactItem>,
    pub conclusions: Vec<ConclusionItem>,
    pub assumptions: Vec<AssumptionItem>,
    pub uncertainties: Vec<UncertaintyItem>,
    pub next_actions: Vec<NextActionItem>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactItem {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConclusionItem {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssumptionItem {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UncertaintyItem {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NextActionItem {
    pub text: String,
}

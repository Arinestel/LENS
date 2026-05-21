use crate::core::engine_selection::{LanguageEngineKind, ReasoningEngineKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreRuntimeConfig {
    pub reasoning_engine: ReasoningEngineKind,
    pub language_engine: LanguageEngineKind,
}

impl CoreRuntimeConfig {
    pub fn new_mock() -> Self {
        Self {
            reasoning_engine: ReasoningEngineKind::Mock,
            language_engine: LanguageEngineKind::Mock,
        }
    }
}

impl Default for CoreRuntimeConfig {
    fn default() -> Self {
        Self::new_mock()
    }
}

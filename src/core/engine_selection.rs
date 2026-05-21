use crate::core::core_runtime_config::CoreRuntimeConfig;
use crate::core::language_engine::LanguageEngine;
use crate::core::language_layer::LanguageLayer;
use crate::core::logic_core::LogicCore;
use crate::core::real_language_config::RealLanguageConfig;
use crate::core::real_language_engine::RealLanguageEngine;
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::real_reasoning_engine::RealReasoningEngine;
use crate::core::reasoning_engine::ReasoningEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEngineKind {
    Mock,
    Real,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageEngineKind {
    Mock,
    Real,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreEngineSelection {
    config: CoreRuntimeConfig,
}

pub struct SelectedCoreEngines {
    pub reasoning: Box<dyn ReasoningEngine>,
    pub language: Box<dyn LanguageEngine>,
}

impl CoreEngineSelection {
    pub fn from_config(config: CoreRuntimeConfig) -> Self {
        Self { config }
    }

    pub fn select_engines(self) -> SelectedCoreEngines {
        SelectedCoreEngines {
            reasoning: self.select_reasoning_engine(),
            language: self.select_language_engine(),
        }
    }

    fn select_reasoning_engine(self) -> Box<dyn ReasoningEngine> {
        match self.config.reasoning_engine {
            ReasoningEngineKind::Mock => Box::new(LogicCore),
            ReasoningEngineKind::Real => Box::new(RealReasoningEngine::new(
                RealReasoningConfig::default(),
            )),
        }
    }

    fn select_language_engine(self) -> Box<dyn LanguageEngine> {
        match self.config.language_engine {
            LanguageEngineKind::Mock => Box::new(LanguageLayer),
            LanguageEngineKind::Real => Box::new(RealLanguageEngine::new(
                RealLanguageConfig::default(),
            )),
        }
    }
}

impl Default for CoreEngineSelection {
    fn default() -> Self {
        Self::from_config(CoreRuntimeConfig::default())
    }
}

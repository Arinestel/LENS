use crate::core::orchestrator::CorePipelineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasoningEngineError {
    ConfigError { message: String },
    NotReady { message: String },
    Timeout { message: String },
    TransportError { message: String },
    InvalidModelOutput { message: String },
    MappingError { message: String },
}

impl ReasoningEngineError {
    pub fn message(&self) -> String {
        match self {
            Self::ConfigError { message } => format!("Reasoning config error: {message}"),
            Self::NotReady { message } => format!("Reasoning engine not ready: {message}"),
            Self::Timeout { message } => format!("Reasoning timeout: {message}"),
            Self::TransportError { message } => format!("Reasoning transport error: {message}"),
            Self::InvalidModelOutput { message } => {
                format!("Reasoning invalid model output: {message}")
            }
            Self::MappingError { message } => format!("Reasoning mapping error: {message}"),
        }
    }
}

impl From<ReasoningEngineError> for CorePipelineError {
    fn from(error: ReasoningEngineError) -> Self {
        Self {
            message: error.message(),
        }
    }
}

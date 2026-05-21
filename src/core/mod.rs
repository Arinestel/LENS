// Цей модуль групує майбутні шари інтеграції моделей LENS App.
// Він лише оголошує файлову структуру для наступного етапу.
// Робоча логіка, pipeline і виклики з UI тут поки що не реалізовані.

pub mod core_input_context;
pub mod core_gateway;
pub mod core_logger;
pub mod core_runtime_config;
pub mod engine_selection;
pub mod language_engine;
pub mod language_layer;
pub mod logic_core;
pub mod orchestrator;
pub mod real_language_config;
pub mod real_language_engine;
pub mod real_reasoning_config;
pub mod real_reasoning_engine;
pub mod reasoning_contract;
pub mod reasoning_engine;
pub mod reasoning_validator;

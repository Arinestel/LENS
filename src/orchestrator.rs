// Цей файл є оркестратором мінімального сценарію LENS Desktop Shell v0.
// Він не містить майбутнього логічного ядра, пам'яті чи інтеграцій:
// зараз лише змінює стан, пише журнал і готує тестову відповідь.

use crate::core::orchestrator::{CoreOrchestrator, OrchestratorRequest, UserQuery};
use crate::logging::Logger;
use crate::state::{AppState, State};

// Виконує той самий сценарій, який запускає кнопка "Launch/Запуск".
pub fn launch_startup_test(state: &mut State, logger: &mut Logger) {
    logger.log_action("Launch command executed");

    // UI бачить, що сценарій почав виконуватися.
    state.set_state(AppState::Processing);
    logger.log_info("Status: Processing");

    // Для першої оболонки відповідь є заглушкою, без моделі й зовнішніх викликів.
    state.update_response("[TEST PLACEHOLDER] LENS Desktop Shell v0 - Response ready.".to_string());
    logger.log_info("Status: Ready");

    // UI може показувати готову відповідь.
    state.set_state(AppState::ShowingResponse);
    logger.log_info("Transitioned to ShowingResponse state");
}

pub fn submit_user_message(state: &mut State, logger: &mut Logger, message: &str) {
    state.set_state(AppState::Processing);
    logger.log_info("Status: Processing");

    let request = OrchestratorRequest::from(UserQuery {
        text: message.to_string(),
        language: "uk".to_string(),
        session_id: None,
    });
    let response = CoreOrchestrator::run_mock_pipeline(request);

    state.update_response(response.user_facing_text);
    logger.log_info("Mock core pipeline response is ready");
    state.set_state(AppState::ShowingResponse);
    logger.log_info("Status: Ready");
}

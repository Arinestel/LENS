// Цей файл зберігає мінімальний стан LENS Desktop Shell v0.
// UI читає ці дані, а оркестратор змінює їх під час сценарію запуску.

// Перелік станів показує, на якому кроці зараз перебуває оболонка.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Boot,
    Ready,
    Processing,
    ShowingResponse,
}

// Основний стан містить тільки те, що потрібно першій версії вікна.
#[derive(Debug, Clone)]
pub struct State {
    pub app_state: AppState,
    pub input: String,
    pub response: String,
    pub lens_response_ready: bool,
    pub dialogue_history: String,
    pub technical_output: String,
}

impl State {
    // Створює початковий стан одразу після відкриття застосунку.
    pub fn new() -> Self {
        Self {
            app_state: AppState::Boot,
            input: String::new(),
            response: String::new(),
            lens_response_ready: false,
            dialogue_history: String::new(),
            technical_output: "LENS Desktop Shell v0 запущено".to_string(),
        }
    }

    // Запам'ятовує текст, який користувач ввів у поле запиту.
    pub fn update_input(&mut self, new_input: String) {
        self.input = new_input;
    }

    pub fn append_user_message(&mut self, message: &str) {
        self.dialogue_history
            .push_str(&format!("User: {}\n", message));
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
    }

    // Запам'ятовує відповідь і готує текст для області діалогу.
    pub fn update_response(&mut self, new_response: String) {
        self.response = new_response;
        self.lens_response_ready = !self.response.trim().is_empty();

        if self.lens_response_ready {
            self.dialogue_history
                .push_str(&format!("LENS: {}\n", self.response));
            self.lens_response_ready = false;
        }
    }

    // Замінює технічний журнал текстом, який буде показано в окремій області.
    pub fn update_technical_output(&mut self, new_output: String) {
        self.technical_output = new_output;
    }

    // Переводить оболонку в інший зрозумілий для UI стан.
    pub fn set_state(&mut self, new_state: AppState) {
        self.app_state = new_state;
    }
}

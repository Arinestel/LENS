// Цей файл відповідає за технічний журнал LENS Desktop Shell v0.
// Журнал живе тільки в пам'яті й потрібен UI для окремої технічної області.

// Logger накопичує короткі записи про дії, стан і помилки оболонки.
pub struct Logger {
    logs: Vec<String>,
}

impl Logger {
    // Створює порожній журнал під час запуску застосунку.
    pub fn new() -> Self {
        Self { logs: Vec::new() }
    }

    // Додає запис і залишає тільки останні 100 рядків, щоб журнал не розростався.
    fn push_limited(&mut self, formatted: String) {
        self.logs.push(formatted);
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    // Записує дію користувача або UI.
    pub fn log_action(&mut self, message: &str) {
        self.push_limited(format!("[ACT] {}", message));
    }

    // Записує звичайне інформаційне повідомлення.
    pub fn log_info(&mut self, message: &str) {
        self.push_limited(format!("[INF] {}", message));
    }

    // Записує помилку, яку потрібно показати в технічному виводі.
    pub fn log_error(&mut self, message: &str) {
        self.push_limited(format!("[ERR] {}", message));
    }

    // Повертає весь журнал одним текстом для відображення в UI.
    pub fn get_logs(&self) -> String {
        if self.logs.is_empty() {
            "Немає логів".to_string()
        } else {
            self.logs
                .iter()
                .rev()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

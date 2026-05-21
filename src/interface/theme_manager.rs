// Цей файл зберігає назву активної теми для локального стану інтерфейсу.
// Кольори теми читаються з ui_config.json, а зовнішні JSON-файли тем
// відкриваються в ui.rs тільки під час зміни теми.

// ThemeManager пам'ятає назву активної теми без додаткових файлових операцій.
#[derive(Debug)]
pub struct ThemeManager {
    theme_name: String,
}

impl ThemeManager {
    // Створює локальний менеджер теми з назви, взятої з ui_config.json.
    pub fn new(theme_name: String) -> Self {
        Self { theme_name }
    }

    // Оновлює назву теми після синхронізації ui_config.json.
    pub fn set_theme(&mut self, theme_name: String) {
        self.theme_name = theme_name;
    }
}

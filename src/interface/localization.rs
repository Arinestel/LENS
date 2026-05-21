// Цей файл відповідає за локальні рядки інтерфейсу.
// Він не читає мовні JSON-файли під час запуску: активні рядки приходять
// з ui_config.json, а зовнішні мовні файли відкриває ui.rs тільки при зміні мови.

use std::collections::HashMap;

// LanguageMetadata описує мову в меню налаштувань.
#[derive(Debug, Clone)]
pub struct LanguageMetadata {
    pub code: String,
    pub language_name: String,
    pub default_font: String,
    pub default_font_size: u16,
}

// LocalizationManager тримає активну мову і вже готову таблицю рядків.
#[derive(Debug)]
pub struct LocalizationManager {
    language_code: String,
    strings: HashMap<String, String>,
}

impl LocalizationManager {
    // Створює менеджер з рядків, які вже завантажені з локального UI-конфіга.
    pub fn new(language_code: String, strings: HashMap<String, String>) -> Self {
        Self {
            language_code,
            strings,
        }
    }

    // Повертає перекладений рядок; якщо ключа немає, показує сам ключ у дужках.
    pub fn get(&self, key: &str) -> String {
        self.strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("[{}]", key))
    }

    // Замінює активну мову й таблицю рядків після того, як ui.rs оновив ui_config.json.
    pub fn apply_language(&mut self, language_code: String, strings: HashMap<String, String>) {
        self.language_code = language_code;
        self.strings = strings;
    }
}

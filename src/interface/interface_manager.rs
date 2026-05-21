// Цей файл керує локальними налаштуваннями інтерфейсу LENS Desktop Shell v0.
// Він працює тільки з даними, які вже були прочитані з ui_config.json.

use crate::interface::localization::LocalizationManager;
use crate::interface::theme_manager::ThemeManager;
use std::collections::HashMap;

// InterfaceSettings описує поточну мову, тему, шрифт і рядки, які бачить UI.
#[derive(Debug, Clone)]
pub struct InterfaceSettings {
    pub language: String,
    pub theme: String,
    pub font_name: String,
    pub font_size: u32,
    pub strings: HashMap<String, String>,
}

impl Default for InterfaceSettings {
    // Дає безпечні початкові налаштування, якщо конфігурація недоступна.
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "light".to_string(),
            font_name: "Times New Roman".to_string(),
            font_size: 14,
            strings: HashMap::new(),
        }
    }
}

// InterfaceManager тримає локальну копію активних UI-налаштувань.
#[derive(Debug)]
pub struct InterfaceManager {
    settings: InterfaceSettings,
    localization: LocalizationManager,
    theme: ThemeManager,
}

impl InterfaceManager {
    // Створює менеджер без читання зовнішніх JSON-файлів.
    pub fn new(settings: InterfaceSettings) -> Self {
        let localization =
            LocalizationManager::new(settings.language.clone(), settings.strings.clone());
        let theme = ThemeManager::new(settings.theme.clone());

        Self {
            settings,
            localization,
            theme,
        }
    }

    // Повертає перекладений рядок за ключем для кнопок, підписів і плейсхолдерів.
    pub fn get_string(&self, key: &str) -> String {
        self.localization.get(key)
    }

    // Застосовує локальні налаштування після оновлення ui_config.json.
    pub fn apply_settings(&mut self, new_settings: InterfaceSettings) {
        if new_settings.language != self.settings.language
            || new_settings.strings != self.settings.strings
        {
            self.localization
                .apply_language(new_settings.language.clone(), new_settings.strings.clone());
        }

        if new_settings.theme != self.settings.theme {
            self.theme.set_theme(new_settings.theme.clone());
        }

        self.settings = new_settings;
    }

    // Дає UI доступ до активних налаштувань без зміни стану.
    pub fn current_settings(&self) -> &InterfaceSettings {
        &self.settings
    }
}

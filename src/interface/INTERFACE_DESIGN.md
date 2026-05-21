# LENS Desktop App Interface Foundation v0.1

## Overview

This document describes the scalable interface foundation for the LENS desktop application. The architecture separates concerns into modular layers: localization, theming, font management, and UI configuration.

---

## 1. Architecture Layers

```
UI Layer (ui.rs)
    ↓
Interface Manager (interface_manager.rs) — coordinates localization, theme, fonts
    ├── Localization Manager (localization.rs)
    ├── Theme Manager (theme_manager.rs)
    ├── Font Manager (font_manager.rs)
    └── UI Metrics Manager (ui_metrics.rs)
    ↓
Resources (JSON files + font files)
```

---

## 2. Data Flow

### 2.1 Application Startup
1. App initializes
2. Load ui_config.json → get default language, theme, font
3. Load language JSON file (e.g., en.json)
4. Load theme JSON file (e.g., light.json)
5. Initialize UI with loaded resources
6. Auto-launch first scenario

### 2.2 User Changes Settings
1. User opens settings panel
2. Selects new language / theme / font / size
3. Settings panel calls InterfaceManager::apply_settings()
4. Manager updates all managers (localization, theme, fonts)
5. UI refreshes with new values
6. Settings persisted (future: to config file)

### 2.3 UI Gets a Label
1. Code calls: localization.get("ui.button_send")
2. Localization manager returns translated string from memory
3. UI widget renders it

---

## 3. JSON File Formats

### 3.1 Language File (languages/en.json)
```json
{
  "metadata": {
    "language_code": "en",
    "language_name": "English",
    "version": "1.0"
  },
  "ui": {
    "button_send": "Send",
    "button_settings": "Settings",
    ...
  },
  "settings": { ... },
  "messages": { ... }
}
```

**Keys are dot-separated paths:** "ui.button_send", "settings.title", etc.

### 3.2 Theme File (themes/light.json)
```json
{
  "metadata": {
    "theme_name": "light",
    "version": "1.0"
  },
  "colors": {
    "background_primary": "#ffffff",
    "text_primary": "#000000",
    ...
  },
  "spacing": {
    "padding_small": 5,
    "padding_medium": 10,
    ...
  },
  "fonts": {
    "default_font": "Segoe UI",
    "default_size": 12,
    ...
  }
}
```

### 3.3 Config File (ui_config.json)
```json
{
  "settings": {
    "default_language": "en",
    "default_theme": "light",
    "default_font": "Segoe UI",
    "default_font_size": 12
  },
  "supported_languages": ["en", "ua"],
  "supported_themes": ["light", "dark"],
  "font_size_range": { "min": 10, "max": 18, "step": 1 }
}
```

---

## 4. Rust Data Structures

### 4.1 Localization Manager
```rust
pub struct LocalizationManager {
    language_code: String,
    strings: Map<String, String>,  // flat map of dot-separated keys
}

impl LocalizationManager {
    pub fn new(language_code: &str) -> Self { ... }
    pub fn load_language(language_code: &str) -> Result<Self, Error> { ... }
    pub fn get(&self, key: &str) -> String { ... }
    pub fn set_language(&mut self, language_code: &str) -> Result<(), Error> { ... }
}
```

### 4.2 Theme Manager
```rust
pub struct ThemeData {
    colors: Map<String, String>,
    spacing: Map<String, f32>,
    fonts: Map<String, serde_json::Value>,
}

pub struct ThemeManager {
    theme_name: String,
    data: ThemeData,
}

impl ThemeManager {
    pub fn new(theme_name: &str) -> Self { ... }
    pub fn load_theme(theme_name: &str) -> Result<Self, Error> { ... }
    pub fn get_color(&self, key: &str) -> String { ... }
    pub fn get_spacing(&self, key: &str) -> f32 { ... }
    pub fn set_theme(&mut self, theme_name: &str) -> Result<(), Error> { ... }
}
```

### 4.3 Font Manager
```rust
pub struct FontSettings {
    pub font_name: String,
    pub font_size: u32,
    pub line_height: f32,  // calculated based on size
}

pub struct FontManager {
    settings: FontSettings,
}

impl FontManager {
    pub fn new(name: String, size: u32) -> Self { ... }
    pub fn set_font(&mut self, name: String, size: u32) { ... }
    pub fn calculate_text_bounds(&self, text: &str) -> (f32, f32) { ... }
    pub fn calculate_button_size(&self, text: &str, padding: f32) -> (f32, f32) { ... }
}
```

### 4.4 Button Sizing Algorithm
```rust
pub fn calculate_button_rectangle(
    text: &str,
    font_size: u32,
    font_name: &str,
    padding_h: f32,  // horizontal padding
    padding_v: f32,  // vertical padding
) -> (f32, f32) {  // (width, height)
    
    // 1. Calculate text bounds using active font and size
    let (text_width, text_height) = estimate_text_bounds(text, font_size, font_name);
    
    // 2. Add padding on each side
    let button_width = text_width + 2.0 * padding_h;
    let button_height = text_height + 2.0 * padding_v;
    
    // 3. Return final button rectangle
    (button_width, button_height)
}
```

### 4.5 Interface Settings Structure
```rust
pub struct InterfaceSettings {
    pub language: String,
    pub theme: String,
    pub font_name: String,
    pub font_size: u32,
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "light".to_string(),
            font_name: "Segoe UI".to_string(),
            font_size: 12,
        }
    }
}
```

### 4.6 Interface Manager (Coordinator)
```rust
pub struct InterfaceManager {
    settings: InterfaceSettings,
    localization: LocalizationManager,
    theme: ThemeManager,
    fonts: FontManager,
}

impl InterfaceManager {
    pub fn new(settings: InterfaceSettings) -> Result<Self, Error> { ... }
    pub fn get_string(&self, key: &str) -> String { ... }
    pub fn get_color(&self, key: &str) -> String { ... }
    pub fn get_spacing(&self, key: &str) -> f32 { ... }
    pub fn apply_settings(&mut self, new_settings: InterfaceSettings) -> Result<(), Error> { ... }
    pub fn current_settings(&self) -> &InterfaceSettings { ... }
}
```

---

## 5. Implementation Plan — Minimal Scalable Foundation

### Phase 0: Structure Only (CURRENT)
- ✅ Create folder structure
- ✅ Define JSON formats
- ✅ Define Rust data structures
- This design document

### Phase 1: Core Managers (NEXT)
1. Create localization.rs with LocalizationManager
2. Create theme_manager.rs with ThemeManager
3. Create font_manager.rs with FontManager
4. Create ui_metrics.rs with ButtonSizing helpers
5. Create interface_manager.rs as coordinator

### Phase 2: First Practical Minimal Step
1. Update App state to include InterfaceManager
2. Add one "Settings" button to the left panel
3. Create a minimal settings panel (slide-out or modal concept)
4. Implement language switching in the settings panel
5. Implement theme switching in the settings panel
6. Verify translations update in real-time
7. Verify theme colors apply to existing panels

### Phase 3: Button Sizing + Future Fonts
1. Implement text bounds estimation
2. Use calculated sizes for buttons instead of hardcoded padding
3. Add font settings UI
4. Add font size range selector (10-18)

---

## 6. Module Responsibilities

| Module | Responsibility | Input | Output |
|--------|-----------------|-------|--------|
| localization.rs | Load language JSON, provide string lookups | key (dot-path) | translated string |
| theme_manager.rs | Load theme JSON, provide color/spacing values | key | color/spacing value |
| font_manager.rs | Track font name/size, estimate text bounds | text + font settings | bounds (w, h) |
| ui_metrics.rs | Helper functions for button sizing, layout calcs | text, font, padding | rectangle (w, h) |
| interface_manager.rs | Coordinate all managers, handle settings changes | new settings | success/error |

---

## 7. Key Design Decisions

1. **JSON-based configuration**: Easy to edit, human-readable, no recompilation needed (future)
2. **Flat key structure with dots**: e.g., "ui.button_send" simplifies nested JSON lookups
3. **Manager separation**: Each manager has one role; composition via InterfaceManager
4. **Language self-names in own language**: "English" in en.json, "Українська" in ua.json
5. **No hardcoded strings in UI code**: All labels come from localization
6. **Modular font system**: Prepared for per-language fonts in future
7. **Button algorithm**: Calculates size from text + padding, not arbitrary fixed sizes

---

## 8. Future Extensions (Not in v0.1)

- Persistent settings storage (JSON file)
- Font file loading and embedding
- Per-language default fonts
- Custom theme creation/editing UI
- Dark theme auto-detection from system
- UI state persistence across sessions
- Accessibility features (font scaling hints)

---

## 9. Notes for Developers

- Keep JSON files in src/interface/languages/ and src/interface/themes/ — must be included in build
- Use InterfaceManager::get_string() for all UI labels
- Use InterfaceManager::get_color() for all colors
- When adding new UI strings, add them to both en.json and ua.json
- When adding new theme colors, add them to both light.json and dark.json
- Always initialize InterfaceManager with default settings early in App::new()
- Settings changes trigger full UI refresh (no partial updates yet)

---

Generated: 2026-04-23
Status: Ready for Phase 1 Implementation

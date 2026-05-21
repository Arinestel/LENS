# Interface Foundation Structure Summary

## Directory Structure Created

```
src/interface/
├── languages/
│   ├── en.json          (English UI strings)
│   └── ua.json          (Ukrainian UI strings)
├── themes/
│   ├── light.json       (Light theme colors & spacing)
│   └── dark.json        (Dark theme colors & spacing)
├── fonts/               (Reserved for future font files)
├── ui_config.json       (Default settings & configuration)
└── INTERFACE_DESIGN.md  (Full architecture documentation)
```

## JSON Files Format Summary

### Language File (en.json / ua.json)
- **Structure**: Nested JSON with metadata + sections
- **Metadata**: language_code, language_name, version
- **Keys**: ui, settings, messages (can be extended)
- **Access pattern**: "section.key" (e.g., "ui.button_send")
- **Self-name rule**: "language_name" must be in own language

### Theme File (light.json / dark.json)
- **Structure**: Flat maps for colors, spacing, fonts
- **Sections**: metadata, colors, spacing, fonts
- **Color naming**: Semantic names (background_primary, text_secondary, accent)
- **Spacing values**: Pixel values as numbers (not strings)
- **Font values**: Object with name and size defaults

### UI Config (ui_config.json)
- **Default values**: language, theme, font, font_size
- **Supported values**: lists of available languages and themes
- **Ranges**: min/max/step for numeric settings

## Rust Data Structures (TO BE IMPLEMENTED)

### Core Managers (Separate Responsibilities)
1. **LocalizationManager** (localization.rs)
   - Loads language JSON
   - Provides string lookups via `get(key: &str)`
   - Allows language switching via `set_language(code: &str)`

2. **ThemeManager** (theme_manager.rs)
   - Loads theme JSON
   - Provides color lookups via `get_color(key: &str)`
   - Provides spacing via `get_spacing(key: &str)`
   - Allows theme switching via `set_theme(name: &str)`

3. **FontManager** (font_manager.rs)
   - Tracks current font name and size
   - Calculates text bounds: `calculate_text_bounds(text: &str)`
   - Prepared for per-language font selection

4. **UI Metrics** (ui_metrics.rs)
   - Button sizing algorithm: `calculate_button_rectangle(text, font_size, font_name, padding_h, padding_v)`
   - Layout helpers (future)

5. **InterfaceManager** (interface_manager.rs)
   - Coordinates all managers
   - Single point of access: `get_string()`, `get_color()`, `get_spacing()`
   - Handles settings changes: `apply_settings(new_settings)`

### Supporting Structures
- **InterfaceSettings**: Holds language, theme, font_name, font_size
- **Language File Structure**: Map<String, String> with dot-separated keys
- **Theme Data**: Maps for colors, spacing, and font references

## Button Sizing Algorithm (To Be Implemented)

```
1. Input: text, font_name, font_size, padding_h, padding_v
2. Calculate: text_bounds(text, font_size, font_name) → (width, height)
3. Add: padding on each side
4. Return: (text_width + 2*padding_h, text_height + 2*padding_v)
5. Render: centered text inside calculated rectangle
```

## How Settings Flow Works

```
User changes setting in UI
    ↓
Settings panel calls: InterfaceManager::apply_settings(new_settings)
    ↓
Manager updates:
    - LocalizationManager::set_language()
    - ThemeManager::set_theme()
    - FontManager::set_font()
    ↓
UI refreshes (full redraw with new strings, colors, fonts)
```

## Internal Identifier Convention

- **Code identifiers**: Always English (button_send, label_dialogue, etc.)
- **Displayed labels**: Come from language JSON file
- **Benefit**: Code readable in English, UI displays in user's language

## Next Implementation Steps

### Phase 1: Core Managers
- [ ] Implement localization.rs (LocalizationManager)
- [ ] Implement theme_manager.rs (ThemeManager)
- [ ] Implement font_manager.rs (FontManager)
- [ ] Implement ui_metrics.rs (ButtonSizing)
- [ ] Implement interface_manager.rs (Coordinator)
- [ ] Update Cargo.toml: add serde_json for JSON parsing

### Phase 2: First Practical Step (Minimal Settings)
- [ ] Add InterfaceManager to App state
- [ ] Add "Settings" button to left panel
- [ ] Create minimal settings panel concept
- [ ] Implement language selector (dropdown: English / Українська)
- [ ] Implement theme selector (dropdown: Light / Dark)
- [ ] Verify translations update in real-time
- [ ] Verify theme colors apply

### Phase 3: Future (Not in v0.1)
- [ ] Font selector UI
- [ ] Font size slider (10-18)
- [ ] Settings persistence to file
- [ ] Font file loading
- [ ] Per-language font defaults

## Files Ready for Review

- `src/interface/languages/en.json` — English strings
- `src/interface/languages/ua.json` — Ukrainian strings
- `src/interface/themes/light.json` — Light theme
- `src/interface/themes/dark.json` — Dark theme
- `src/interface/ui_config.json` — Configuration
- `src/interface/INTERFACE_DESIGN.md` — Full architecture details
- `src/interface/STRUCTURE.md` — This summary

---

**Status**: Ready for Phase 1 (Core Managers Implementation)
**Last Updated**: 2026-04-23

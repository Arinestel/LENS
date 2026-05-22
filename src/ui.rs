// Модуль інтерфейсу користувача для LENS Desktop Shell v0.
// Основна роль: побудова головного вікна, обробка натискань кнопок
// та показ діалогової області, логів і меню налаштувань.
//
// Важливий інваріант цього файла:
// відкриття меню кнопок не повинно змінювати геометрію головного макета.
// Меню має з'являтися поруч із кнопкою-предком,
// а не як окремий відривний блок унизу або вгорі вікна.

use iced::{
    alignment::Horizontal,
    event, keyboard,
    widget::{button, column, container, mouse_area, row, scrollable, text, text_editor},
    Application, Background, Color, Command, Element, Font, Length, Padding, Subscription, Theme,
};

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::core::core_gateway::{CoreGateway, UiCoreRequest};
use crate::core::core_logger::CoreLogger;
use crate::core::core_runtime_config::CoreRuntimeConfig;
use crate::core::orchestrator::CoreOrchestrator;
use crate::core::real_reasoning_config::RealReasoningConfig;
use crate::core::reasoning_contract::ReasoningResult;
use crate::core::reasoning_readiness::ReasoningReadinessStatus;
use crate::interface::interface_manager::{InterfaceManager, InterfaceSettings};
use crate::interface::localization::LanguageMetadata;
use crate::logging::Logger;
use crate::orchestrator;
use crate::server_control::{
    check_server_one_model_status, check_server_one_status, start_server_one_if_needed,
    ServerOneModelStatus, ServerOneStatus,
};
use crate::state::{AppState, State};

#[cfg(target_os = "windows")]
const WINDOWS_VIRTUAL_KEY_V: i32 = 0x56;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
}

fn is_paste_shortcut(key: keyboard::Key<&str>, modifiers: keyboard::Modifiers) -> bool {
    modifiers.control()
        && (matches!(key, keyboard::Key::Character("v" | "V")) || is_physical_paste_key_pressed())
}

#[cfg(target_os = "windows")]
fn is_physical_paste_key_pressed() -> bool {
    unsafe { (GetAsyncKeyState(WINDOWS_VIRTUAL_KEY_V) as u16 & 0x8000) != 0 }
}

#[cfg(not(target_os = "windows"))]
fn is_physical_paste_key_pressed() -> bool {
    false
}

// Базові розміри UI тримаються тут, щоб макет лишався передбачуваним.
const DEFAULT_UI_FONT_SIZE: u16 = 14;

const BUTTON_HORIZONTAL_OFFSET: f32 = 10.0;

const BUTTON_VERTICAL_OFFSET: f32 = 10.0;

const INPUT_HORIZONTAL_PADDING: f32 = 10.0;

const INPUT_VERTICAL_PADDING: f32 = 6.0;

const INPUT_VISIBLE_LINES: f32 = 3.0;

const CONTROL_SEPARATOR_VERTICAL_PADDING: f32 = 5.0;

// Накладне меню прив'язується до кнопки без зміни геометрії основного вікна.
const MENU_HORIZONTAL_ATTACH_RATIO: f32 = 0.85;
const MENU_VERTICAL_ATTACH_RATIO: f32 = 0.3;

const SETTINGS_FIRST_MENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_THEME_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_LANGUAGE_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_FONT_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_AI_MODELS_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_MESSAGES_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const SETTINGS_INPUT_SUBMENU_CLOSE_DELAY_MS: u64 = 250;
const DEBUG_MENU_CLOSE_DELAY_MS: u64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputSubmitShortcut {
    Enter,
    EnterCtrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelIndicatorState {
    Empty,
    ConfiguredDisconnected,
    Connected,
}

impl InputSubmitShortcut {
    fn matches_enter(self, control_pressed: bool) -> bool {
        match self {
            Self::Enter => !control_pressed,
            Self::EnterCtrl => control_pressed,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Enter => "Enter",
            Self::EnterCtrl => "Enter+Ctrl",
        }
    }
}

// UiColorRgba зберігає колір у форматі, зручному для JSON-конфігурації.
#[derive(Debug, Clone, Copy, PartialEq)]
struct UiColorRgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl UiColorRgba {
    // Створює колір з окремих каналів червоного, зеленого, синього і прозорості.
    fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        UiColorRgba { r, g, b, a }
    }

    // Перетворює локальний формат кольору у формат бібліотеки iced.
    fn to_iced_color(&self) -> Color {
        Color::from_rgba(self.r, self.g, self.b, self.a)
    }
}

// UiThemeColors містить усі кольори, які потрібні для малювання поточного вікна.
#[derive(Debug, Clone)]
struct UiThemeColors {
    window_background: UiColorRgba,
    left_panel_background: UiColorRgba,
    logs_background: UiColorRgba,
    logs_text_color: UiColorRgba,
    dialogue_background: UiColorRgba,
    dialogue_text_color: UiColorRgba,
    dialogue_user_nickname_color: UiColorRgba,
    dialogue_lens_nickname_color: UiColorRgba,
    input_background: UiColorRgba,
    input_text_color: UiColorRgba,
    button_normal_background: UiColorRgba,
    button_hover_background: UiColorRgba,
    button_active_background: UiColorRgba,
    button_text_normal_color: UiColorRgba,
    button_text_hover_color: UiColorRgba,
    button_text_active_color: UiColorRgba,
    menu_background: UiColorRgba,
    menu_hover_background: UiColorRgba,
    menu_border: UiColorRgba,
    primary_text_color: UiColorRgba,
    secondary_text_color: UiColorRgba,
    separator_color: UiColorRgba,
    server_one_button_running_background: UiColorRgba,
    server_one_button_running_hover_background: UiColorRgba,
    server_one_button_running_active_background: UiColorRgba,
    server_one_button_not_running_background: UiColorRgba,
    server_one_button_not_running_hover_background: UiColorRgba,
    server_one_button_not_running_active_background: UiColorRgba,
    server_one_button_text_color: UiColorRgba,
    server_one_indicator_idle_fill: UiColorRgba,
    server_one_indicator_border: UiColorRgba,
}

// UiTheme поєднує назву теми і її палітру.
#[derive(Debug, Clone)]
struct UiTheme {
    name: String,
    colors: UiThemeColors,
}

// ThemedButtonStyle описує вигляд звичайної кнопки в різних станах.
#[derive(Debug, Clone, Copy)]
struct ThemedButtonStyle {
    normal_background: UiColorRgba,
    hover_background: UiColorRgba,
    active_background: UiColorRgba,
    normal_text: UiColorRgba,
    hover_text: UiColorRgba,
    active_text: UiColorRgba,
    border_color: Option<UiColorRgba>,
}

#[derive(Debug, Clone)]
struct ServerControlButtonStyle {
    status: ServerOneStatus,
    colors: UiThemeColors,
}

// ThemedMenuButtonStyle додає до кнопки стан, потрібний для пунктів меню.
#[derive(Debug, Clone, Copy)]
struct ThemedMenuButtonStyle {
    button: ThemedButtonStyle,
    visual_state: MenuButtonVisualState,
}

// MenuButtonVisualState показує, як саме має виглядати пункт меню зараз.
#[derive(Debug, Clone, Copy)]
enum MenuButtonVisualState {
    Normal,
    Hover,
    Active,
}

impl ThemedButtonStyle {
    // Будує стиль кнопки з активної палітри теми.
    fn from_colors(colors: &UiThemeColors) -> Self {
        ThemedButtonStyle {
            normal_background: colors.button_normal_background,
            hover_background: colors.button_hover_background,
            active_background: colors.button_active_background,
            normal_text: colors.button_text_normal_color,
            hover_text: colors.button_text_hover_color,
            active_text: colors.button_text_active_color,
            border_color: None,
        }
    }

    // Готує опис вигляду кнопки для iced.
    fn appearance(
        border_color: Option<UiColorRgba>,
        background: UiColorRgba,
        text_color: UiColorRgba,
    ) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Background::Color(background.to_iced_color())),
            text_color: text_color.to_iced_color(),
            border: iced::Border {
                width: if border_color.is_some() { 2.0 } else { 0.0 },
                color: border_color
                    .map(|color| color.to_iced_color())
                    .unwrap_or(Color::TRANSPARENT),
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

impl ServerControlButtonStyle {
    fn new(status: ServerOneStatus, colors: &UiThemeColors) -> Self {
        Self {
            status,
            colors: colors.clone(),
        }
    }

    fn colors(&self) -> (UiColorRgba, UiColorRgba, UiColorRgba) {
        match self.status {
            ServerOneStatus::Running => (
                self.colors.server_one_button_running_background,
                self.colors.server_one_button_running_hover_background,
                self.colors.server_one_button_running_active_background,
            ),
            ServerOneStatus::NotRunning => (
                self.colors.server_one_button_not_running_background,
                self.colors.server_one_button_not_running_hover_background,
                self.colors.server_one_button_not_running_active_background,
            ),
        }
    }

    fn appearance(&self, background: UiColorRgba) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Background::Color(background.to_iced_color())),
            text_color: self.colors.server_one_button_text_color.to_iced_color(),
            border: iced::Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

impl ThemedMenuButtonStyle {
    // Будує стиль пункту меню з активної палітри й поточного стану наведення.
    fn from_colors_and_state(colors: &UiThemeColors, visual_state: MenuButtonVisualState) -> Self {
        let menu_background = UiColorRgba {
            a: 1.0,
            ..colors.menu_background
        };
        let menu_hover_background = UiColorRgba {
            a: 1.0,
            ..colors.menu_hover_background
        };

        ThemedMenuButtonStyle {
            button: ThemedButtonStyle {
                normal_background: menu_background,
                hover_background: menu_hover_background,
                active_background: menu_background,
                normal_text: colors.button_text_normal_color,
                hover_text: colors.button_text_hover_color,
                active_text: colors.button_text_active_color,
                border_color: Some(UiColorRgba {
                    a: 1.0,
                    ..colors.button_text_normal_color
                }),
            },
            visual_state,
        }
    }

    // Вибирає потрібний вигляд меню: звичайний, наведений або активний.
    fn appearance_for_state(
        &self,
        visual_state: MenuButtonVisualState,
    ) -> iced::widget::button::Appearance {
        match visual_state {
            MenuButtonVisualState::Normal => Self::filled_menu_button_appearance(
                self.button.border_color,
                self.button.normal_background,
                self.button.normal_text,
            ),
            MenuButtonVisualState::Hover => Self::filled_menu_button_appearance(
                self.button.border_color,
                self.button.hover_background,
                self.button.hover_text,
            ),
            MenuButtonVisualState::Active => Self::filled_menu_button_appearance(
                self.button.border_color,
                self.button.active_background,
                self.button.active_text,
            ),
        }
    }

    // Робить фон меню непрозорим, щоб текст не змішувався з основним вікном.
    fn filled_menu_button_appearance(
        border_color: Option<UiColorRgba>,
        background: UiColorRgba,
        text_color: UiColorRgba,
    ) -> iced::widget::button::Appearance {
        let filled_background = UiColorRgba {
            a: 1.0,
            ..background
        };

        iced::widget::button::Appearance {
            background: Some(Background::Color(filled_background.to_iced_color())),
            text_color: text_color.to_iced_color(),
            border: iced::Border {
                width: if border_color.is_some() { 2.0 } else { 0.0 },
                color: border_color
                    .map(|color| color.to_iced_color())
                    .unwrap_or(Color::TRANSPARENT),
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

impl iced::widget::button::StyleSheet for ThemedButtonStyle {
    type Style = Theme;

    // Повертає вигляд кнопки у спокійному стані.
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        Self::appearance(self.border_color, self.normal_background, self.normal_text)
    }

    // Повертає вигляд кнопки, коли курсор над нею.
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        Self::appearance(self.border_color, self.hover_background, self.hover_text)
    }

    // Повертає вигляд кнопки під час натискання.
    fn pressed(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        Self::appearance(self.border_color, self.active_background, self.active_text)
    }
}

impl iced::widget::button::StyleSheet for ServerControlButtonStyle {
    type Style = Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        let (normal, _, _) = self.colors();
        self.appearance(normal)
    }

    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        let (_, hover, _) = self.colors();
        self.appearance(hover)
    }

    fn pressed(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        let (_, _, active) = self.colors();
        self.appearance(active)
    }
}

impl iced::widget::button::StyleSheet for ThemedMenuButtonStyle {
    type Style = Theme;

    // Повертає вигляд пункту меню відповідно до його збереженого стану.
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        self.appearance_for_state(self.visual_state)
    }

    // Повертає вигляд пункту меню під курсором.
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        self.appearance_for_state(MenuButtonVisualState::Hover)
    }

    // Повертає вигляд пункту меню під час натискання.
    fn pressed(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        self.appearance_for_state(MenuButtonVisualState::Active)
    }
}

// ThemedTextInputStyle описує поле введення згідно з активною темою.
#[derive(Debug, Clone, Copy)]
struct ThemedTextInputStyle {
    background: UiColorRgba,
    text: UiColorRgba,
    placeholder: UiColorRgba,
    selection: UiColorRgba,
}

impl ThemedTextInputStyle {
    // Створює стиль поля введення з кольорів теми.
    fn from_colors(colors: &UiThemeColors) -> Self {
        ThemedTextInputStyle {
            background: colors.input_background,
            text: colors.input_text_color,
            placeholder: colors.secondary_text_color,
            selection: colors.primary_text_color,
        }
    }

    // Готує вигляд поля введення для iced.
    fn appearance(&self) -> iced::widget::text_input::Appearance {
        iced::widget::text_input::Appearance {
            background: Background::Color(self.background.to_iced_color()),
            border: iced::Border {
                radius: 2.0.into(),
                width: 1.0,
                color: Color::TRANSPARENT,
            },
            icon_color: self.text.to_iced_color(),
        }
    }

    fn editor_appearance(&self) -> iced::widget::text_editor::Appearance {
        iced::widget::text_editor::Appearance {
            background: Background::Color(self.background.to_iced_color()),
            border: iced::Border {
                radius: 2.0.into(),
                width: 1.0,
                color: Color::TRANSPARENT,
            },
        }
    }
}

struct InputTextHighlighter {
    color: UiColorRgba,
}

impl iced::advanced::text::Highlighter for InputTextHighlighter {
    type Settings = UiColorRgba;
    type Highlight = UiColorRgba;
    type Iterator<'a> = std::option::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        InputTextHighlighter { color: *settings }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.color = *new_settings;
    }

    fn change_line(&mut self, _line: usize) {}

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if line.is_empty() {
            None
        } else {
            Some((0..line.len(), self.color))
        }
        .into_iter()
    }

    fn current_line(&self) -> usize {
        0
    }
}

fn input_text_format(
    highlight: &UiColorRgba,
    _theme: &Theme,
) -> iced::advanced::text::highlighter::Format<Font> {
    iced::advanced::text::highlighter::Format {
        color: Some(highlight.to_iced_color()),
        font: None,
    }
}

impl iced::widget::text_input::StyleSheet for ThemedTextInputStyle {
    type Style = Theme;

    // Повертає вигляд поля, коли воно доступне для введення.
    fn active(&self, _style: &Self::Style) -> iced::widget::text_input::Appearance {
        self.appearance()
    }

    // Повертає вигляд поля, коли в ньому стоїть курсор.
    fn focused(&self, _style: &Self::Style) -> iced::widget::text_input::Appearance {
        self.appearance()
    }

    // Повертає вигляд поля, коли воно вимкнене.
    fn disabled(&self, _style: &Self::Style) -> iced::widget::text_input::Appearance {
        self.appearance()
    }

    // Задає колір підказки всередині порожнього поля.
    fn placeholder_color(&self, _style: &Self::Style) -> Color {
        self.placeholder.to_iced_color()
    }

    // Задає колір введеного користувачем тексту.
    fn value_color(&self, _style: &Self::Style) -> Color {
        self.text.to_iced_color()
    }

    // Задає колір тексту для вимкненого поля.
    fn disabled_color(&self, _style: &Self::Style) -> Color {
        self.text.to_iced_color()
    }

    // Задає колір виділення тексту в полі.
    fn selection_color(&self, _style: &Self::Style) -> Color {
        self.selection.to_iced_color()
    }
}

impl iced::widget::text_editor::StyleSheet for ThemedTextInputStyle {
    type Style = Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::text_editor::Appearance {
        self.editor_appearance()
    }

    fn focused(&self, _style: &Self::Style) -> iced::widget::text_editor::Appearance {
        self.editor_appearance()
    }

    fn placeholder_color(&self, _style: &Self::Style) -> Color {
        self.placeholder.to_iced_color()
    }

    fn value_color(&self, _style: &Self::Style) -> Color {
        self.text.to_iced_color()
    }

    fn disabled_color(&self, _style: &Self::Style) -> Color {
        self.text.to_iced_color()
    }

    fn selection_color(&self, _style: &Self::Style) -> Color {
        self.selection.to_iced_color()
    }

    fn disabled(&self, _style: &Self::Style) -> iced::widget::text_editor::Appearance {
        self.editor_appearance()
    }
}

impl UiTheme {
    // Повертає світлу тему, яка використовується як безпечний запасний варіант.
    fn default_light() -> Self {
        UiTheme {
            name: "light".to_string(),
            colors: UiThemeColors {
                window_background: UiColorRgba::new(1.0, 1.0, 1.0, 1.0),
                left_panel_background: UiColorRgba::new(0.95, 0.95, 0.95, 1.0),
                logs_background: UiColorRgba::new(0.98, 0.98, 0.98, 1.0),
                logs_text_color: UiColorRgba::new(0.2, 0.2, 0.2, 1.0),
                dialogue_background: UiColorRgba::new(1.0, 1.0, 1.0, 1.0),
                dialogue_text_color: UiColorRgba::new(0.2, 0.2, 0.2, 1.0),
                dialogue_user_nickname_color: UiColorRgba::new(0.0, 0.5, 1.0, 1.0),
                dialogue_lens_nickname_color: UiColorRgba::new(0.5, 0.5, 0.5, 1.0),
                input_background: UiColorRgba::new(1.0, 1.0, 1.0, 1.0),
                input_text_color: UiColorRgba::new(0.2, 0.2, 0.2, 1.0),
                button_normal_background: UiColorRgba::new(0.9, 0.9, 0.9, 1.0),
                button_hover_background: UiColorRgba::new(0.8, 0.8, 0.8, 1.0),
                button_active_background: UiColorRgba::new(0.7, 0.7, 0.7, 1.0),
                button_text_normal_color: UiColorRgba::new(0.0, 0.0, 0.0, 1.0),
                button_text_hover_color: UiColorRgba::new(0.0, 0.0, 0.0, 1.0),
                button_text_active_color: UiColorRgba::new(0.0, 0.0, 0.0, 1.0),
                menu_background: UiColorRgba::new(0.12, 0.12, 0.15, 1.0),
                menu_hover_background: UiColorRgba::new(0.56, 0.56, 0.575, 1.0),
                menu_border: UiColorRgba::new(0.08, 0.08, 0.10, 1.0),
                primary_text_color: UiColorRgba::new(0.1, 0.1, 0.1, 1.0),
                secondary_text_color: UiColorRgba::new(0.5, 0.5, 0.5, 1.0),
                separator_color: UiColorRgba::new(0.8, 0.8, 0.8, 1.0),
                server_one_button_running_background: UiColorRgba::new(0.08, 0.46, 0.18, 1.0),
                server_one_button_running_hover_background: UiColorRgba::new(0.10, 0.58, 0.23, 1.0),
                server_one_button_running_active_background: UiColorRgba::new(
                    0.05, 0.36, 0.13, 1.0,
                ),
                server_one_button_not_running_background: UiColorRgba::new(0.66, 0.10, 0.10, 1.0),
                server_one_button_not_running_hover_background: UiColorRgba::new(
                    0.78, 0.13, 0.13, 1.0,
                ),
                server_one_button_not_running_active_background: UiColorRgba::new(
                    0.50, 0.07, 0.07, 1.0,
                ),
                server_one_button_text_color: UiColorRgba::new(1.0, 1.0, 1.0, 1.0),
                server_one_indicator_idle_fill: UiColorRgba::new(0.45, 0.45, 0.45, 1.0),
                server_one_indicator_border: UiColorRgba::new(0.02, 0.08, 0.22, 1.0),
            },
        }
    }
}

// SettingsButtonMetrics зберігає розмір кнопки налаштувань для позиціювання меню.
#[derive(Debug, Clone, Copy)]
struct SettingsButtonMetrics {
    width: f32,
    height: f32,
}

// ButtonGeometry описує ширину й висоту кнопки перед її малюванням.
#[derive(Debug, Clone, Copy)]
struct ButtonGeometry {
    width: f32,
    height: f32,
}

// ButtonPlacement пояснює, чи кнопка стоїть окремо, чи всередині меню.
#[derive(Debug, Clone, Copy)]
enum ButtonPlacement {
    Standalone,
    MenuPanel,
}

// MenuButtonTextBinding зберігає шрифт і розмір тексту для пункту меню.
#[derive(Debug, Clone, Copy)]
struct MenuButtonTextBinding {
    font_name: Option<&'static str>,
    font_size: u16,
}

// Перевіряє, чи будь-яка частина гілки меню зараз активна.
fn menu_branch_active(active_flags: &[bool]) -> bool {
    active_flags.iter().any(|active| *active)
}

// Вирішує, чи треба почати відкладене закриття меню.
fn menu_should_start_delayed_close(
    node_open: bool,
    close_pending: bool,
    branch_active: bool,
) -> bool {
    node_open && !branch_active && !close_pending
}

// Перевіряє, чи минув час відкладеного закриття меню.
fn menu_pending_close_elapsed(
    close_pending_since: Option<Instant>,
    now: Instant,
    delay: Duration,
) -> bool {
    close_pending_since
        .map(|started_at| now.duration_since(started_at) >= delay)
        .unwrap_or(false)
}

// LocalMenuNodeState описує тільки наведення, відкриття та відкладене закриття меню.
// Це не є станом сценарію LENS.
#[derive(Debug, Clone, Copy)]
struct LocalMenuNodeState {
    parent_active: bool,
    zone_active: bool,
    open: bool,
    close_pending: bool,
    close_pending_since: Option<Instant>,
}

impl LocalMenuNodeState {
    // Створює закритий пункт меню без активного наведення.
    fn closed() -> Self {
        LocalMenuNodeState {
            parent_active: false,
            zone_active: false,
            open: false,
            close_pending: false,
            close_pending_since: None,
        }
    }

    // Перемикає меню після натискання на батьківську кнопку.
    fn toggle_open_from_parent(&mut self) {
        self.open = !self.open;
        self.parent_active = self.open;
        self.zone_active = false;
        self.close_pending = false;
        self.close_pending_since = None;
    }

    // Закриває меню негайно і скидає всі ознаки наведення.
    fn close_now(&mut self) {
        self.parent_active = false;
        self.zone_active = false;
        self.open = false;
        self.close_pending = false;
        self.close_pending_since = None;
    }

    // Каже, чи меню зараз відкрите.
    fn is_open(&self) -> bool {
        self.open
    }

    // Каже, чи меню очікує таймер перед закриттям.
    fn has_pending_close(&self) -> bool {
        self.close_pending
    }

    // Оновлює наведення на батьківський елемент меню.
    fn set_parent_active(
        &mut self,
        active: bool,
        now: Instant,
        extra_active_flags: &[bool],
        open_when_branch_active: bool,
    ) {
        self.parent_active = active;
        self.refresh_open_state(now, extra_active_flags, open_when_branch_active);
    }

    // Оновлює наведення на область самого меню.
    fn set_zone_active(
        &mut self,
        active: bool,
        now: Instant,
        extra_active_flags: &[bool],
        open_when_branch_active: bool,
    ) {
        self.zone_active = active;
        self.refresh_open_state(now, extra_active_flags, open_when_branch_active);
    }

    // Закриває меню, якщо відкладений таймер уже спрацював.
    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        if !self.close_pending {
            return false;
        }

        if self.close_pending_since.is_none() {
            self.close_pending = false;
            return false;
        }

        if menu_pending_close_elapsed(self.close_pending_since, now, delay) {
            self.close_now();
            return true;
        }

        false
    }

    // Перераховує, чи меню має лишатися відкритим.
    fn refresh_open_state(
        &mut self,
        now: Instant,
        extra_active_flags: &[bool],
        open_when_branch_active: bool,
    ) {
        let branch_active = self.branch_active(extra_active_flags);

        if branch_active {
            if open_when_branch_active {
                self.open = true;
            }
            self.close_pending = false;
            self.close_pending_since = None;
        } else if menu_should_start_delayed_close(self.open, self.close_pending, branch_active) {
            self.close_pending = true;
            self.close_pending_since = Some(now);
        } else if !self.open {
            self.close_pending = false;
            self.close_pending_since = None;
        }
    }

    // Повертає активність цілої гілки меню разом із дочірніми ознаками.
    fn branch_active(&self, extra_active_flags: &[bool]) -> bool {
        let local_active = menu_branch_active(&[self.parent_active, self.zone_active]);
        local_active || menu_branch_active(extra_active_flags)
    }
}

// SettingsFirstMenuState керує першим рівнем меню налаштувань.
#[derive(Debug, Clone, Copy)]
struct SettingsFirstMenuState {
    node: LocalMenuNodeState,
}

impl SettingsFirstMenuState {
    // Створює закритий перший рівень меню.
    fn closed() -> Self {
        SettingsFirstMenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    // Перемикає перше меню після натискання кнопки налаштувань.
    fn toggle_open(&mut self) {
        self.node.toggle_open_from_parent();
    }

    // Відкриває меню, коли користувач навів курсор на кнопку налаштувань.
    fn open_from_parent_hover(&mut self, now: Instant) {
        self.node.set_parent_active(true, now, &[], true);
    }

    // Закриває перше меню негайно.
    fn close_now(&mut self) {
        self.node.close_now();
    }

    // Каже, чи перше меню відкрите.
    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    // Каже, чи перше меню чекає перед закриттям.
    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    // Передає стан наведення на кнопку налаштувань.
    fn set_parent_button_active(&mut self, active: bool, now: Instant, child_branch_active: bool) {
        self.node
            .set_parent_active(active, now, &[child_branch_active], false);
    }

    // Передає стан наведення на область першого меню.
    fn set_menu_zone_active(&mut self, active: bool, now: Instant, child_branch_active: bool) {
        self.node
            .set_zone_active(active, now, &[child_branch_active], false);
    }

    // Закриває перше меню, якщо таймер закриття завершився.
    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

// SettingsThemeSubmenuState керує підменю вибору теми.
#[derive(Debug, Clone, Copy)]
struct DebugMenuState {
    node: LocalMenuNodeState,
}

impl DebugMenuState {
    fn closed() -> Self {
        DebugMenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    fn toggle_open(&mut self) {
        self.node.toggle_open_from_parent();
    }

    fn open_from_parent_hover(&mut self, now: Instant) {
        self.node.set_parent_active(true, now, &[], true);
    }

    fn close_now(&mut self) {
        self.node.close_now();
    }

    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    fn set_parent_button_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], false);
    }

    fn set_menu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], false);
    }

    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

#[derive(Debug, Clone, Copy)]
struct SettingsThemeSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsThemeSubmenuState {
    // Створює закрите підменю теми.
    fn closed() -> Self {
        SettingsThemeSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    // Закриває підменю теми негайно.
    fn close_now(&mut self) {
        self.node.close_now();
    }

    // Каже, чи підменю теми чекає перед закриттям.
    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    // Каже, чи підменю теми відкрите.
    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    // Каже, чи активна гілка теми.
    fn branch_active(&self) -> bool {
        self.node.branch_active(&[])
    }

    // Каже, чи підменю теми має утримувати батьківське меню відкритим.
    fn keeps_parent_branch_open(&self) -> bool {
        self.branch_active() || self.is_open() || self.has_pending_close()
    }

    // Передає стан наведення на пункт "тема".
    fn set_parent_item_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], true);
    }

    // Передає стан наведення на область підменю теми.
    fn set_submenu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], true);
    }

    // Закриває підменю теми, якщо таймер закриття завершився.
    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

// SettingsLanguageSubmenuState керує підменю вибору мови.
#[derive(Debug, Clone, Copy)]
struct SettingsLanguageSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsLanguageSubmenuState {
    // Створює закрите підменю мови.
    fn closed() -> Self {
        SettingsLanguageSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    // Каже, чи підменю мови відкрите.
    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    // Каже, чи активна гілка мови.
    fn branch_active(&self) -> bool {
        self.node.branch_active(&[])
    }

    // Каже, чи підменю мови має утримувати батьківське меню відкритим.
    fn keeps_parent_branch_open(&self) -> bool {
        self.branch_active() || self.is_open() || self.has_pending_close()
    }

    // Закриває підменю мови негайно.
    fn close_now(&mut self) {
        self.node.close_now();
    }

    // Каже, чи підменю мови чекає перед закриттям.
    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    // Передає стан наведення на пункт "мова".
    fn set_parent_item_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], true);
    }

    // Передає стан наведення на область підменю мови.
    fn set_submenu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], true);
    }

    // Закриває підменю мови, якщо таймер закриття завершився.
    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

// SettingsFontSubmenuState керує підменю налаштувань шрифту.
#[derive(Debug, Clone, Copy)]
struct SettingsFontSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsFontSubmenuState {
    // Створює закрите підменю шрифту.
    fn closed() -> Self {
        SettingsFontSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    // Каже, чи підменю шрифту відкрите.
    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    // Каже, чи активна гілка шрифту.
    fn branch_active(&self) -> bool {
        self.node.branch_active(&[])
    }

    // Каже, чи підменю шрифту має утримувати батьківське меню відкритим.
    fn keeps_parent_branch_open(&self) -> bool {
        self.branch_active() || self.is_open() || self.has_pending_close()
    }

    // Закриває підменю шрифту негайно.
    fn close_now(&mut self) {
        self.node.close_now();
    }

    // Каже, чи підменю шрифту чекає перед закриттям.
    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    // Передає стан наведення на пункт "шрифт".
    fn set_parent_item_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], true);
    }

    // Передає стан наведення на область підменю шрифту.
    fn set_submenu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], true);
    }

    // Закриває підменю шрифту, якщо таймер закриття завершився.
    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

// MenuOverlayState об'єднує стан усіх меню налаштувань.
#[derive(Debug, Clone, Copy)]
struct SettingsAiModelsSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsAiModelsSubmenuState {
    fn closed() -> Self {
        SettingsAiModelsSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    fn branch_active(&self) -> bool {
        self.node.branch_active(&[])
    }

    fn keeps_parent_branch_open(&self) -> bool {
        self.branch_active() || self.is_open() || self.has_pending_close()
    }

    fn close_now(&mut self) {
        self.node.close_now();
    }

    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    fn set_parent_item_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], true);
    }

    fn set_submenu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], true);
    }

    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

#[derive(Debug, Clone, Copy)]
struct SettingsMessagesSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsMessagesSubmenuState {
    fn closed() -> Self {
        SettingsMessagesSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    fn branch_active(&self, input_branch_active: bool) -> bool {
        self.node.branch_active(&[input_branch_active])
    }

    fn keeps_parent_branch_open(&self, input_branch_active: bool) -> bool {
        self.branch_active(input_branch_active) || self.is_open() || self.has_pending_close()
    }

    fn close_now(&mut self) {
        self.node.close_now();
    }

    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    fn set_parent_item_active(&mut self, active: bool, now: Instant, input_branch_active: bool) {
        self.node
            .set_parent_active(active, now, &[input_branch_active], true);
    }

    fn set_submenu_zone_active(&mut self, active: bool, now: Instant, input_branch_active: bool) {
        self.node
            .set_zone_active(active, now, &[input_branch_active], true);
    }

    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

#[derive(Debug, Clone, Copy)]
struct SettingsInputSubmenuState {
    node: LocalMenuNodeState,
}

impl SettingsInputSubmenuState {
    fn closed() -> Self {
        SettingsInputSubmenuState {
            node: LocalMenuNodeState::closed(),
        }
    }

    fn is_open(&self) -> bool {
        self.node.is_open()
    }

    fn branch_active(&self) -> bool {
        self.node.branch_active(&[])
    }

    fn keeps_parent_branch_open(&self) -> bool {
        self.branch_active() || self.is_open() || self.has_pending_close()
    }

    fn close_now(&mut self) {
        self.node.close_now();
    }

    fn has_pending_close(&self) -> bool {
        self.node.has_pending_close()
    }

    fn set_parent_item_active(&mut self, active: bool, now: Instant) {
        self.node.set_parent_active(active, now, &[], true);
    }

    fn set_submenu_zone_active(&mut self, active: bool, now: Instant) {
        self.node.set_zone_active(active, now, &[], true);
    }

    fn close_if_pending_expired(&mut self, now: Instant, delay: Duration) -> bool {
        self.node.close_if_pending_expired(now, delay)
    }
}

#[derive(Debug, Clone, Copy)]
struct MenuOverlayState {
    settings_first_menu: SettingsFirstMenuState,
    settings_theme_submenu: SettingsThemeSubmenuState,
    settings_language_submenu: SettingsLanguageSubmenuState,
    settings_font_submenu: SettingsFontSubmenuState,
    settings_ai_models_submenu: SettingsAiModelsSubmenuState,
    settings_messages_submenu: SettingsMessagesSubmenuState,
    settings_input_submenu: SettingsInputSubmenuState,
    debug_menu: DebugMenuState,
}

impl MenuOverlayState {
    // Створює повністю закриту систему меню.
    fn closed() -> Self {
        MenuOverlayState {
            settings_first_menu: SettingsFirstMenuState::closed(),
            settings_theme_submenu: SettingsThemeSubmenuState::closed(),
            settings_language_submenu: SettingsLanguageSubmenuState::closed(),
            settings_font_submenu: SettingsFontSubmenuState::closed(),
            settings_ai_models_submenu: SettingsAiModelsSubmenuState::closed(),
            settings_messages_submenu: SettingsMessagesSubmenuState::closed(),
            settings_input_submenu: SettingsInputSubmenuState::closed(),
            debug_menu: DebugMenuState::closed(),
        }
    }

    fn close_settings_branch(&mut self) {
        self.settings_first_menu.close_now();
        self.settings_theme_submenu.close_now();
        self.settings_language_submenu.close_now();
        self.settings_font_submenu.close_now();
        self.settings_ai_models_submenu.close_now();
        self.settings_messages_submenu.close_now();
        self.settings_input_submenu.close_now();
    }

    // Каже, чи будь-яке дочірнє меню налаштувань зараз активне.
    fn settings_child_branch_active(&self) -> bool {
        let input_branch_active = self.settings_input_submenu.keeps_parent_branch_open();
        self.settings_theme_submenu.keeps_parent_branch_open()
            || self.settings_language_submenu.keeps_parent_branch_open()
            || self.settings_font_submenu.keeps_parent_branch_open()
            || self.settings_ai_models_submenu.keeps_parent_branch_open()
            || self
                .settings_messages_submenu
                .keeps_parent_branch_open(input_branch_active)
            || input_branch_active
    }

    // Оновлює наведення на кнопку налаштувань з урахуванням дочірніх меню.
    fn set_settings_parent_button_active(&mut self, active: bool, now: Instant) {
        let child_branch_active = self.settings_child_branch_active();
        self.settings_first_menu
            .set_parent_button_active(active, now, child_branch_active);
    }

    // Оновлює наведення на перший рівень меню з урахуванням дочірніх меню.
    fn set_settings_menu_zone_active(&mut self, active: bool, now: Instant) {
        let child_branch_active = self.settings_child_branch_active();
        self.settings_first_menu
            .set_menu_zone_active(active, now, child_branch_active);
    }

    // Після закриття підменю оновлює перший рівень меню.
    fn refresh_settings_menu_branch(&mut self, now: Instant) {
        let menu_zone_active = self.settings_first_menu.node.zone_active;
        self.set_settings_menu_zone_active(menu_zone_active, now);
    }
}

// LanguageSubmenuItem описує один пункт меню вибору мови.
#[derive(Debug, Clone)]
struct LanguageSubmenuItem {
    key: String,
    label: String,
    font_name: String,
    font_size: u16,
}

// ThemeSubmenuItem описує один пункт меню вибору теми.
#[derive(Debug, Clone)]
struct ThemeSubmenuItem {
    key: String,
    label: String,
}

// FontSubmenuItem описує один пункт підменю шрифту.
#[derive(Debug, Clone)]
struct FontSubmenuItem {
    key: String,
    label: String,
}

#[derive(Debug, Clone)]
struct BasicSubmenuItem {
    key: String,
    label: String,
}

#[derive(Debug, Clone)]
struct SettingsFirstMenuItem {
    key: &'static str,
    label: String,
}

// SettingsOverlayMenuLevel показує, який рівень меню треба намалювати.
#[derive(Debug, Clone, Copy)]
enum SettingsOverlayMenuLevel {
    First,
    ThemeSubmenu,
    LanguageSubmenu,
    FontSubmenu,
    AiModelsSubmenu,
    MessagesSubmenu,
    InputSubmenu,
}

// SettingsOverlayMenuPanel потрібен для таймерів закриття конкретних меню.
#[derive(Debug, Clone, Copy)]
enum SettingsOverlayMenuPanel {
    First,
    ThemeSubmenu,
    LanguageSubmenu,
    FontSubmenu,
    AiModelsSubmenu,
    MessagesSubmenu,
    InputSubmenu,
}

// SettingsOverlayNestedMenuScene описує позицію і розмір вкладеного меню.
#[derive(Debug, Clone, Copy)]
struct SettingsOverlayNestedMenuScene {
    level: SettingsOverlayMenuLevel,
    width: f32,
    horizontal_overlap: f32,
    top_offset: f32,
}

// SettingsOverlayScene описує всю видиму сцену меню налаштувань.
#[derive(Debug, Clone, Copy)]
struct SettingsOverlayScene {
    first_menu_width: f32,
    nested_menu: Option<SettingsOverlayNestedMenuScene>,
    child_nested_menu: Option<SettingsOverlayNestedMenuScene>,
}

// LanguageTextBinding зберігає шрифт, потрібний для поточної мови.
#[derive(Debug, Clone)]
struct LanguageTextBinding {
    font_name: String,
    font_size: u16,
}

impl LanguageTextBinding {
    // Перетворює назву шрифту у формат iced.
    fn font(&self) -> Font {
        Font::with_name(intern_font_name(&self.font_name))
    }

    // Готує прив'язку тексту для кнопок меню.
    fn menu_button_text_binding(&self) -> MenuButtonTextBinding {
        MenuButtonTextBinding {
            font_name: Some(intern_font_name(&self.font_name)),
            font_size: self.font_size,
        }
    }
}

// Зберігає назву шрифту в статичній пам'яті, бо iced очікує довгоживучий рядок.
fn intern_font_name(font_name: &str) -> &'static str {
    static FONT_NAME_CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

    let cache = FONT_NAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().expect("font name cache mutex poisoned");

    if let Some(cached_font_name) = cache.get(font_name) {
        return *cached_font_name;
    }

    let static_font_name = Box::leak(font_name.to_string().into_boxed_str());
    cache.insert(font_name.to_string(), static_font_name);
    static_font_name
}

// Message описує події інтерфейсу. Запуск сценарію делегується оркестратору.
#[derive(Debug, Clone)]
pub enum Message {
    InputEdited(text_editor::Action),
    SendPressed,
    KeyboardEnterPressed { control: bool },
    KeyboardPastePressed,
    ClipboardTextRead(Option<String>),
    SettingsPressed,
    DebugPressed,
    ServerOnePressed,
    MenuButtonNoop,
    SettingsMenuItemSelected(String),
    DebugMenuItemSelected(String),
    SettingsParentEntered,
    SettingsParentExited,
    SettingsMenuEntered,
    SettingsMenuExited,
    SettingsCloseDelayTick(Instant),
    DebugParentEntered,
    DebugParentExited,
    DebugMenuEntered,
    DebugMenuExited,
    DebugCloseDelayTick(Instant),
    SettingsThemeParentEntered,
    SettingsThemeParentExited,
    SettingsThemeSubmenuEntered,
    SettingsThemeSubmenuExited,
    SettingsThemeCloseDelayTick(Instant),
    SettingsLanguageParentEntered,
    SettingsLanguageParentExited,
    SettingsLanguageSubmenuEntered,
    SettingsLanguageSubmenuExited,
    SettingsLanguageCloseDelayTick(Instant),
    SettingsFontParentEntered,
    SettingsFontParentExited,
    SettingsFontSubmenuEntered,
    SettingsFontSubmenuExited,
    SettingsFontCloseDelayTick(Instant),
    SettingsAiModelsParentEntered,
    SettingsAiModelsParentExited,
    SettingsAiModelsSubmenuEntered,
    SettingsAiModelsSubmenuExited,
    SettingsAiModelsCloseDelayTick(Instant),
    SettingsMessagesParentEntered,
    SettingsMessagesParentExited,
    SettingsMessagesSubmenuEntered,
    SettingsMessagesSubmenuExited,
    SettingsMessagesCloseDelayTick(Instant),
    SettingsInputParentEntered,
    SettingsInputParentExited,
    SettingsInputSubmenuEntered,
    SettingsInputSubmenuExited,
    SettingsInputCloseDelayTick(Instant),
}

// App є мінімальною оболонкою: UI, стан застосунку, журнал і менеджер інтерфейсу.
pub struct App {
    state: State,
    logger: Logger,
    interface: InterfaceManager,
    overlay_menus: MenuOverlayState,
    input_content: text_editor::Content,
    dialogue_scroll_id: iced::widget::scrollable::Id,
    input_scroll_id: iced::widget::scrollable::Id,
    skip_next_editor_enter: bool,
    skip_next_editor_paste: bool,
    submit_shortcut: InputSubmitShortcut,
    server_one_status: ServerOneStatus,
    reasoning_model_indicator_state: ModelIndicatorState,
}

impl App {
    fn app_resource_path(relative_path: impl AsRef<Path>) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path)
    }

    fn input_content_text(&self) -> String {
        self.input_content
            .text()
            .strip_suffix('\n')
            .unwrap_or("")
            .to_string()
    }

    fn scroll_input_to_bottom(&self) -> Command<Message> {
        iced::widget::scrollable::snap_to(
            self.input_scroll_id.clone(),
            iced::widget::scrollable::RelativeOffset { x: 0.0, y: 1.0 },
        )
    }

    fn scroll_dialogue_to_bottom(&self) -> Command<Message> {
        iced::widget::scrollable::snap_to(
            self.dialogue_scroll_id.clone(),
            iced::widget::scrollable::RelativeOffset { x: 0.0, y: 1.0 },
        )
    }

    fn submit_current_input(&mut self, source: &str) {
        let message = self.state.input.trim().to_string();

        if message.is_empty() {
            self.logger.log_info(&format!(
                "Message send via {} ignored - input is empty",
                source
            ));
            self.state.update_technical_output(self.logger.get_logs());
            return;
        }

        self.logger
            .log_action(&format!("Message sent via {}", source));
        self.state.append_user_message(&message);
        self.state.clear_input();
        self.input_content = text_editor::Content::new();
        orchestrator::submit_user_message(&mut self.state, &mut self.logger, &message);
        self.logger
            .log_info("Message accepted, waiting for LENS response");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn paste_plain_text_into_input(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        self.input_content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(text),
            )));
        self.state.update_input(self.input_content_text());
        self.logger.log_info("Plain text pasted into input field");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn show_reasoning_readiness(&mut self) {
        self.logger.log_action("Debug readiness button clicked");

        let runtime_config = CoreRuntimeConfig::default();
        let readiness = CoreOrchestrator::check_reasoning_readiness(&runtime_config);
        let response_text = Self::format_reasoning_readiness_status(&readiness);

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        self.logger.log_info("Reasoning readiness result shown");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn show_active_runtime_config(&mut self) {
        self.logger
            .log_action("Debug runtime config button clicked");

        let response_text = match Self::active_runtime_config() {
            Ok(runtime_config) => Self::format_active_runtime_config(&runtime_config),
            Err(error) => format!("Active runtime config error: {error}"),
        };

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        self.logger.log_info("Active runtime config shown");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn refresh_server_one_status_for_button(&mut self) {
        match check_server_one_status() {
            Ok(status) => {
                self.server_one_status = status;
                self.logger.log_info(match status {
                    ServerOneStatus::Running => "Server 1 status checked: Ollama is running",
                    ServerOneStatus::NotRunning => "Server 1 status checked: Ollama is not running",
                });
            }
            Err(error) => {
                self.server_one_status = ServerOneStatus::NotRunning;
                self.logger
                    .log_error(&format!("Server 1 status check failed: {error}"));
            }
        }
    }

    fn refresh_reasoning_model_indicator_state(&mut self) {
        let real_config = RealReasoningConfig::default();
        let model_name = real_config.model_name.trim();

        if model_name.is_empty() {
            self.reasoning_model_indicator_state = ModelIndicatorState::Empty;
            self.logger
                .log_info("Reasoning model indicator checked: model name is empty");
            return;
        }

        self.reasoning_model_indicator_state = match check_server_one_model_status(model_name) {
            Ok(ServerOneModelStatus::Available) => ModelIndicatorState::Connected,
            Ok(ServerOneModelStatus::Unavailable) | Err(_) => {
                ModelIndicatorState::ConfiguredDisconnected
            }
        };
        self.logger
            .log_info(match self.reasoning_model_indicator_state {
                ModelIndicatorState::Empty => {
                    "Reasoning model indicator checked: model name is empty"
                }
                ModelIndicatorState::ConfiguredDisconnected => {
                    "Reasoning model indicator checked: configured but disconnected"
                }
                ModelIndicatorState::Connected => "Reasoning model indicator checked: connected",
            });
    }

    fn model_indicator_color(state: ModelIndicatorState, colors: &UiThemeColors) -> Color {
        match state {
            ModelIndicatorState::Empty => colors.server_one_indicator_idle_fill.to_iced_color(),
            ModelIndicatorState::ConfiguredDisconnected => colors
                .server_one_button_not_running_background
                .to_iced_color(),
            ModelIndicatorState::Connected => {
                colors.server_one_button_running_background.to_iced_color()
            }
        }
    }

    fn show_server_one_error(&mut self, message: String) {
        self.state
            .update_response(format!("Сервер 1 error: {message}"));
        self.state.set_state(AppState::ShowingResponse);
        self.logger
            .log_error(&format!("Server 1 controlled error: {message}"));
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn handle_server_one_pressed(&mut self) {
        self.logger
            .log_action("Server 1 button clicked - Ollama local server control");

        match check_server_one_status() {
            Ok(ServerOneStatus::Running) => {
                self.server_one_status = ServerOneStatus::Running;
                self.logger
                    .log_info("Server 1 check completed: Ollama is already running");
                self.state.update_technical_output(self.logger.get_logs());
            }
            Ok(ServerOneStatus::NotRunning) => match start_server_one_if_needed() {
                Ok(status) => {
                    self.server_one_status = status;
                    self.logger
                        .log_info("Server 1 start requested and status refreshed");
                    self.state.update_technical_output(self.logger.get_logs());
                }
                Err(error) => {
                    self.server_one_status = ServerOneStatus::NotRunning;
                    self.show_server_one_error(error.to_string());
                }
            },
            Err(error) => {
                self.server_one_status = ServerOneStatus::NotRunning;
                self.show_server_one_error(error.to_string());
            }
        }

        self.refresh_reasoning_model_indicator_state();
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn run_mock_reasoning_test(&mut self) {
        self.logger
            .log_action("Debug mock reasoning test button clicked");

        let input_text = self.input_content_text().trim().to_string();
        if input_text.is_empty() {
            self.state.update_response(
                "reasoning source: Mock\nlanguage source: Mock\n\nmock reasoning test error: input is empty"
                    .to_string(),
            );
            self.state.set_state(AppState::ShowingResponse);
            CoreLogger::log("debug_mock_reasoning_test", "blocked: input is empty");
            self.logger
                .log_info("Mock reasoning test blocked - input is empty");
            self.state.update_technical_output(self.logger.get_logs());
            return;
        }

        CoreLogger::log("debug_mock_reasoning_test", "started");

        let settings = self.interface.current_settings();
        let request = UiCoreRequest {
            text: input_text,
            language: settings.language.clone(),
            session_id: None,
            branch_id: None,
            user_id: None,
        };

        let response = CoreGateway::run_mock_pipeline(request);
        let response_text = match response.error {
            Some(error) if response.response_text.trim().is_empty() => format!(
                "reasoning source: Mock\nlanguage source: Mock\n\nmock reasoning test error: {error}"
            ),
            Some(_) => response.response_text,
            None if response.response_text.trim().is_empty() => {
                "reasoning source: Mock\nlanguage source: Mock\n\nmock reasoning test error: empty response"
                    .to_string()
            }
            None => response.response_text,
        };

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        CoreLogger::log("debug_mock_reasoning_test", "completed");
        self.logger.log_info("Mock reasoning test result shown");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn run_manual_real_reasoning_test(&mut self) {
        self.logger
            .log_action("Debug manual real reasoning test clicked");

        let input_text = self.input_content_text().trim().to_string();
        if input_text.is_empty() {
            self.state.update_response(
                "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\nmanual real reasoning test error: input is empty".to_string(),
            );
            self.state.set_state(AppState::ShowingResponse);
            self.logger
                .log_info("Manual real reasoning test blocked - input is empty");
            self.state.update_technical_output(self.logger.get_logs());
            return;
        }

        let settings = self.interface.current_settings();
        let request = UiCoreRequest {
            text: input_text,
            language: settings.language.clone(),
            session_id: None,
            branch_id: None,
            user_id: None,
        };

        let response =
            CoreGateway::run_manual_real_reasoning_test(request, RealReasoningConfig::default());
        let response_text = match response.error {
            Some(error) if response.response_text.trim().is_empty() => format!(
                "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\nmanual real reasoning test error: {error}"
            ),
            Some(_) => response.response_text,
            None if response.response_text.trim().is_empty() => {
                "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Mock\n\nmanual real reasoning test error: empty response".to_string()
            }
            None => response.response_text,
        };

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        self.logger
            .log_info("Manual real reasoning test result shown");
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn show_raw_reasoning_result(&mut self) {
        self.logger
            .log_action("Debug raw reasoning result requested");

        let input_text = self.input_content_text().trim().to_string();
        if input_text.is_empty() {
            self.state
                .update_response(Self::format_raw_reasoning_error("input is empty"));
            self.state.set_state(AppState::ShowingResponse);
            self.logger
                .log_info("Raw reasoning result blocked - input is empty");
            self.state.update_technical_output(self.logger.get_logs());
            return;
        }

        let settings = self.interface.current_settings();
        let request = UiCoreRequest {
            text: input_text,
            language: settings.language.clone(),
            session_id: None,
            branch_id: None,
            user_id: None,
        };

        let response =
            CoreGateway::run_manual_raw_reasoning_result(request, RealReasoningConfig::default());
        let response_text = match (&response.error, &response.reasoning_result) {
            (Some(error), _) => Self::format_raw_reasoning_error(error),
            (None, Some(reasoning_result)) => Self::format_raw_reasoning_result(reasoning_result),
            (None, None) => Self::format_raw_reasoning_error("empty reasoning result"),
        };

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        if response.error.is_some() {
            self.logger.log_error("Raw reasoning result failed");
        } else {
            self.logger.log_info("Raw reasoning result shown");
        }
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn open_core_log(&mut self) {
        self.logger.log_action("Debug open core log requested");

        let log_path = CoreLogger::log_path();
        let response_text = if !log_path.exists() {
            self.logger.log_error("Core log file does not exist");
            format!(
                "Core log error: file does not exist\npath: {}",
                log_path.display()
            )
        } else {
            match fs::read_to_string(&log_path) {
                Ok(content) => {
                    self.logger.log_info("Core log content shown");
                    format!("Core log path: {}\n\n{}", log_path.display(), content)
                }
                Err(error) => {
                    self.logger.log_error("Core log file could not be read");
                    format!(
                        "Core log error: could not read file\npath: {}\nerror: {}",
                        log_path.display(),
                        error
                    )
                }
            }
        };

        self.state.update_response(response_text);
        self.state.set_state(AppState::ShowingResponse);
        self.state.update_technical_output(self.logger.get_logs());
    }

    fn active_runtime_config() -> Result<CoreRuntimeConfig, String> {
        Ok(CoreRuntimeConfig::default())
    }

    fn format_active_runtime_config(runtime_config: &CoreRuntimeConfig) -> String {
        format!(
            "Active runtime config:\nreasoning_engine: {:?}\nlanguage_engine: {:?}",
            runtime_config.reasoning_engine, runtime_config.language_engine
        )
    }

    fn format_reasoning_readiness_status(readiness: &ReasoningReadinessStatus) -> String {
        match readiness {
            ReasoningReadinessStatus::Ready => "Reasoning readiness: ready".to_string(),
            ReasoningReadinessStatus::ConfigIncomplete { reason } => {
                format!("Reasoning readiness error: config incomplete - {reason}")
            }
        }
    }

    fn format_raw_reasoning_result(reasoning_result: &ReasoningResult) -> String {
        format!(
            "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Not used\npayload source: core reasoning result\n\ntask:\n{}\n\nfacts:\n{}\n\nconclusions:\n{}\n\nassumptions:\n{}\n\nuncertainties:\n{}\n\nnext_actions:\n{}\n\nconfidence:\n{:.2}",
            reasoning_result.task,
            Self::format_raw_text_items(
                reasoning_result
                    .facts
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_raw_text_items(
                reasoning_result
                    .conclusions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_raw_text_items(
                reasoning_result
                    .assumptions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_raw_text_items(
                reasoning_result
                    .uncertainties
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            Self::format_raw_text_items(
                reasoning_result
                    .next_actions
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect()
            ),
            reasoning_result.confidence,
        )
    }

    fn format_raw_text_items(items: Vec<&str>) -> String {
        if items.is_empty() {
            return "- none".to_string();
        }

        let mut formatted = String::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                formatted.push('\n');
            }
            formatted.push_str("- ");
            formatted.push_str(item);
        }
        formatted
    }

    fn format_raw_reasoning_error(error: impl AsRef<str>) -> String {
        format!(
            "reasoning source: Real\nreasoning backend: Ollama\nlanguage source: Not used\npayload source: core reasoning result\n\nraw reasoning result error: {}",
            error.as_ref()
        )
    }

    fn settings_first_menu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(SETTINGS_FIRST_MENU_CLOSE_DELAY_MS));
                Instant::now()
            },
            Message::SettingsCloseDelayTick,
        )
    }

    fn settings_theme_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(SETTINGS_THEME_SUBMENU_CLOSE_DELAY_MS));
                Instant::now()
            },
            Message::SettingsThemeCloseDelayTick,
        )
    }

    fn settings_language_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(
                    SETTINGS_LANGUAGE_SUBMENU_CLOSE_DELAY_MS,
                ));
                Instant::now()
            },
            Message::SettingsLanguageCloseDelayTick,
        )
    }

    fn settings_font_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(SETTINGS_FONT_SUBMENU_CLOSE_DELAY_MS));
                Instant::now()
            },
            Message::SettingsFontCloseDelayTick,
        )
    }

    fn settings_ai_models_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(
                    SETTINGS_AI_MODELS_SUBMENU_CLOSE_DELAY_MS,
                ));
                Instant::now()
            },
            Message::SettingsAiModelsCloseDelayTick,
        )
    }

    fn settings_messages_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(
                    SETTINGS_MESSAGES_SUBMENU_CLOSE_DELAY_MS,
                ));
                Instant::now()
            },
            Message::SettingsMessagesCloseDelayTick,
        )
    }

    fn settings_input_submenu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(SETTINGS_INPUT_SUBMENU_CLOSE_DELAY_MS));
                Instant::now()
            },
            Message::SettingsInputCloseDelayTick,
        )
    }

    fn debug_menu_close_delay_command() -> Command<Message> {
        Command::perform(
            async {
                std::thread::sleep(Duration::from_millis(DEBUG_MENU_CLOSE_DELAY_MS));
                Instant::now()
            },
            Message::DebugCloseDelayTick,
        )
    }

    fn settings_overlay_menu_close_delay_command(
        panel: SettingsOverlayMenuPanel,
    ) -> Command<Message> {
        match panel {
            SettingsOverlayMenuPanel::First => Self::settings_first_menu_close_delay_command(),
            SettingsOverlayMenuPanel::ThemeSubmenu => {
                Self::settings_theme_submenu_close_delay_command()
            }
            SettingsOverlayMenuPanel::LanguageSubmenu => {
                Self::settings_language_submenu_close_delay_command()
            }
            SettingsOverlayMenuPanel::FontSubmenu => {
                Self::settings_font_submenu_close_delay_command()
            }
            SettingsOverlayMenuPanel::AiModelsSubmenu => {
                Self::settings_ai_models_submenu_close_delay_command()
            }
            SettingsOverlayMenuPanel::MessagesSubmenu => {
                Self::settings_messages_submenu_close_delay_command()
            }
            SettingsOverlayMenuPanel::InputSubmenu => {
                Self::settings_input_submenu_close_delay_command()
            }
        }
    }

    fn settings_overlay_menu_has_pending_close(&self, panel: SettingsOverlayMenuPanel) -> bool {
        match panel {
            SettingsOverlayMenuPanel::First => {
                self.overlay_menus.settings_first_menu.has_pending_close()
            }
            SettingsOverlayMenuPanel::ThemeSubmenu => self
                .overlay_menus
                .settings_theme_submenu
                .has_pending_close(),
            SettingsOverlayMenuPanel::LanguageSubmenu => self
                .overlay_menus
                .settings_language_submenu
                .has_pending_close(),
            SettingsOverlayMenuPanel::FontSubmenu => {
                self.overlay_menus.settings_font_submenu.has_pending_close()
            }
            SettingsOverlayMenuPanel::AiModelsSubmenu => self
                .overlay_menus
                .settings_ai_models_submenu
                .has_pending_close(),
            SettingsOverlayMenuPanel::MessagesSubmenu => self
                .overlay_menus
                .settings_messages_submenu
                .has_pending_close(),
            SettingsOverlayMenuPanel::InputSubmenu => self
                .overlay_menus
                .settings_input_submenu
                .has_pending_close(),
        }
    }

    fn settings_overlay_pending_close_command(
        &self,
        panels: &[SettingsOverlayMenuPanel],
    ) -> Option<Command<Message>> {
        let commands = panels
            .iter()
            .copied()
            .filter(|panel| self.settings_overlay_menu_has_pending_close(*panel))
            .map(Self::settings_overlay_menu_close_delay_command)
            .collect::<Vec<_>>();

        if commands.is_empty() {
            None
        } else {
            Some(Command::batch(commands))
        }
    }

    fn settings_overlay_menu_close_if_pending_expired(
        &mut self,
        panel: SettingsOverlayMenuPanel,
        now: Instant,
    ) -> bool {
        match panel {
            SettingsOverlayMenuPanel::First => self
                .overlay_menus
                .settings_first_menu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_FIRST_MENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::ThemeSubmenu => self
                .overlay_menus
                .settings_theme_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_THEME_SUBMENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::LanguageSubmenu => self
                .overlay_menus
                .settings_language_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_LANGUAGE_SUBMENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::FontSubmenu => self
                .overlay_menus
                .settings_font_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_FONT_SUBMENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::AiModelsSubmenu => self
                .overlay_menus
                .settings_ai_models_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_AI_MODELS_SUBMENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::MessagesSubmenu => self
                .overlay_menus
                .settings_messages_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_MESSAGES_SUBMENU_CLOSE_DELAY_MS),
                ),
            SettingsOverlayMenuPanel::InputSubmenu => self
                .overlay_menus
                .settings_input_submenu
                .close_if_pending_expired(
                    now,
                    Duration::from_millis(SETTINGS_INPUT_SUBMENU_CLOSE_DELAY_MS),
                ),
        }
    }

    fn theme_colors_from_json(colors_obj: &serde_json::Value) -> UiThemeColors {
        let extract_color = |key: &str| -> UiColorRgba {
            if let Some(color_obj) = colors_obj.get(key) {
                let r = color_obj.get("r").and_then(|v| v.as_f64()).unwrap_or(0.95) as f32;
                let g = color_obj.get("g").and_then(|v| v.as_f64()).unwrap_or(0.95) as f32;
                let b = color_obj.get("b").and_then(|v| v.as_f64()).unwrap_or(0.95) as f32;
                let a = color_obj.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                UiColorRgba::new(r, g, b, a)
            } else {
                UiTheme::default_light().colors.window_background
            }
        };

        UiThemeColors {
            window_background: extract_color("window_background"),
            left_panel_background: extract_color("left_panel_background"),
            logs_background: extract_color("logs_background"),
            logs_text_color: extract_color("logs_text_color"),
            dialogue_background: extract_color("dialogue_background"),
            dialogue_text_color: extract_color("dialogue_text_color"),
            dialogue_user_nickname_color: extract_color("dialogue_user_nickname_color"),
            dialogue_lens_nickname_color: extract_color("dialogue_lens_nickname_color"),
            input_background: extract_color("input_background"),
            input_text_color: extract_color("input_text_color"),
            button_normal_background: extract_color("button_normal_background"),
            button_hover_background: extract_color("button_hover_background"),
            button_active_background: extract_color("button_active_background"),
            button_text_normal_color: extract_color("button_text_normal_color"),
            button_text_hover_color: extract_color("button_text_hover_color"),
            button_text_active_color: extract_color("button_text_active_color"),
            menu_background: extract_color("menu_background"),
            menu_hover_background: extract_color("menu_hover_background"),
            menu_border: extract_color("menu_border"),
            primary_text_color: extract_color("primary_text_color"),
            secondary_text_color: extract_color("secondary_text_color"),
            separator_color: extract_color("separator_color"),
            server_one_button_running_background: extract_color(
                "server_one_button_running_background",
            ),
            server_one_button_running_hover_background: extract_color(
                "server_one_button_running_hover_background",
            ),
            server_one_button_running_active_background: extract_color(
                "server_one_button_running_active_background",
            ),
            server_one_button_not_running_background: extract_color(
                "server_one_button_not_running_background",
            ),
            server_one_button_not_running_hover_background: extract_color(
                "server_one_button_not_running_hover_background",
            ),
            server_one_button_not_running_active_background: extract_color(
                "server_one_button_not_running_active_background",
            ),
            server_one_button_text_color: extract_color("server_one_button_text_color"),
            server_one_indicator_idle_fill: extract_color("server_one_indicator_idle_fill"),
            server_one_indicator_border: extract_color("server_one_indicator_border"),
        }
    }

    fn read_external_theme_file(theme_path: &str) -> Result<UiTheme, Box<dyn std::error::Error>> {
        let theme_text = std::fs::read_to_string(Self::app_resource_path(theme_path))?;
        let theme_json = serde_json::from_str::<serde_json::Value>(&theme_text)?;

        let colors_obj = theme_json
            .get("current")
            .and_then(|current| current.get("colors"))
            .or_else(|| theme_json.get("colors"));

        let colors_obj = colors_obj.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "external theme file is missing colors",
            )
        })?;

        let theme_name = theme_json
            .get("current")
            .and_then(|current| current.get("theme"))
            .and_then(|theme| theme.as_str())
            .or_else(|| theme_json.get("theme").and_then(|theme| theme.as_str()))
            .or_else(|| theme_json.get("name").and_then(|name| name.as_str()))
            .unwrap_or("external")
            .to_string();

        Ok(UiTheme {
            name: theme_name,
            colors: Self::theme_colors_from_json(colors_obj),
        })
    }

    fn color_to_json(color: UiColorRgba) -> serde_json::Value {
        serde_json::json!({
            "r": color.r,
            "g": color.g,
            "b": color.b,
            "a": color.a
        })
    }

    fn theme_colors_to_json(colors: &UiThemeColors) -> serde_json::Value {
        serde_json::json!({
            "window_background": Self::color_to_json(colors.window_background),
            "left_panel_background": Self::color_to_json(colors.left_panel_background),
            "logs_background": Self::color_to_json(colors.logs_background),
            "logs_text_color": Self::color_to_json(colors.logs_text_color),
            "dialogue_background": Self::color_to_json(colors.dialogue_background),
            "dialogue_text_color": Self::color_to_json(colors.dialogue_text_color),
            "dialogue_user_nickname_color": Self::color_to_json(colors.dialogue_user_nickname_color),
            "dialogue_lens_nickname_color": Self::color_to_json(colors.dialogue_lens_nickname_color),
            "input_background": Self::color_to_json(colors.input_background),
            "input_text_color": Self::color_to_json(colors.input_text_color),
            "button_normal_background": Self::color_to_json(colors.button_normal_background),
            "button_hover_background": Self::color_to_json(colors.button_hover_background),
            "button_active_background": Self::color_to_json(colors.button_active_background),
            "button_text_normal_color": Self::color_to_json(colors.button_text_normal_color),
            "button_text_hover_color": Self::color_to_json(colors.button_text_hover_color),
            "button_text_active_color": Self::color_to_json(colors.button_text_active_color),
            "menu_background": Self::color_to_json(colors.menu_background),
            "menu_hover_background": Self::color_to_json(colors.menu_hover_background),
            "menu_border": Self::color_to_json(colors.menu_border),
            "primary_text_color": Self::color_to_json(colors.primary_text_color),
            "secondary_text_color": Self::color_to_json(colors.secondary_text_color),
            "separator_color": Self::color_to_json(colors.separator_color),
            "server_one_button_running_background": Self::color_to_json(colors.server_one_button_running_background),
            "server_one_button_running_hover_background": Self::color_to_json(colors.server_one_button_running_hover_background),
            "server_one_button_running_active_background": Self::color_to_json(colors.server_one_button_running_active_background),
            "server_one_button_not_running_background": Self::color_to_json(colors.server_one_button_not_running_background),
            "server_one_button_not_running_hover_background": Self::color_to_json(colors.server_one_button_not_running_hover_background),
            "server_one_button_not_running_active_background": Self::color_to_json(colors.server_one_button_not_running_active_background),
            "server_one_button_text_color": Self::color_to_json(colors.server_one_button_text_color),
            "server_one_indicator_idle_fill": Self::color_to_json(colors.server_one_indicator_idle_fill),
            "server_one_indicator_border": Self::color_to_json(colors.server_one_indicator_border)
        })
    }

    // Читає локальний UI-конфіг, який є єдиним джерелом активних налаштувань.
    fn read_ui_config() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let config_text =
            std::fs::read_to_string(Self::app_resource_path("src/interface/ui_config.json"))?;
        Ok(serde_json::from_str::<serde_json::Value>(&config_text)?)
    }

    // Записує оновлений локальний UI-конфіг після зміни мови або теми.
    fn write_ui_config(config_json: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        let updated_config_text = serde_json::to_string_pretty(config_json)?;
        std::fs::write(
            Self::app_resource_path("src/interface/ui_config.json"),
            updated_config_text,
        )?;
        Ok(())
    }

    // Перетворює вкладений JSON із мовного файлу на прості ключі для ui_config.json.
    fn collect_localized_strings(
        prefix: &str,
        value: &serde_json::Value,
        strings: &mut HashMap<String, String>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child_value) in map {
                    let child_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::collect_localized_strings(&child_prefix, child_value, strings);
                }
            }
            serde_json::Value::String(text) => {
                strings.insert(prefix.to_string(), text.clone());
            }
            _ => {}
        }
    }

    // Дістає активні рядки з ui_config.json без читання мовних файлів.
    fn strings_from_config(config_json: &serde_json::Value) -> HashMap<String, String> {
        let mut strings = HashMap::new();

        if let Some(strings_obj) = config_json
            .get("current")
            .and_then(|current| current.get("strings"))
        {
            Self::collect_localized_strings("", strings_obj, &mut strings);
        }

        strings
    }

    // Читає метадані мов з ui_config.json, щоб меню не відкривало папку мов.
    fn language_metadata_from_config() -> Vec<LanguageMetadata> {
        Self::read_ui_config()
            .ok()
            .and_then(|config_json| {
                config_json
                    .get("metadata")
                    .and_then(|metadata| metadata.get("available_languages"))
                    .and_then(|languages| languages.as_array())
                    .map(|languages| {
                        languages
                            .iter()
                            .filter_map(|language| {
                                let code = language.get("code")?.as_str()?.to_string();
                                let language_name =
                                    language.get("language_name")?.as_str()?.to_string();
                                let default_font =
                                    language.get("default_font")?.as_str()?.to_string();
                                let default_font_size = language
                                    .get("default_font_size")?
                                    .as_u64()
                                    .and_then(|value| u16::try_from(value).ok())?;

                                Some(LanguageMetadata {
                                    code,
                                    language_name,
                                    default_font,
                                    default_font_size,
                                })
                            })
                            .collect()
                    })
            })
            .unwrap_or_default()
    }

    // Читає список тем з ui_config.json для побудови меню тем.
    fn theme_items_from_config(&self) -> Vec<ThemeSubmenuItem> {
        Self::read_ui_config()
            .ok()
            .and_then(|config_json| {
                config_json
                    .get("metadata")
                    .and_then(|metadata| metadata.get("available_themes"))
                    .and_then(|themes| themes.as_array())
                    .map(|themes| {
                        themes
                            .iter()
                            .filter_map(|theme| {
                                let key = theme.get("key")?.as_str()?.to_string();
                                let label_key = theme.get("label_key")?.as_str()?;
                                let fallback = theme
                                    .get("fallback_label")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or(key.as_str());
                                let label = self.localized_string_or(label_key, fallback);

                                Some(ThemeSubmenuItem { key, label })
                            })
                            .collect()
                    })
            })
            .unwrap_or_else(|| {
                vec![
                    ThemeSubmenuItem {
                        key: "light".to_string(),
                        label: self.localized_string_or("settings.option_light", "Light"),
                    },
                    ThemeSubmenuItem {
                        key: "dark".to_string(),
                        label: self.localized_string_or("settings.option_dark", "Dark"),
                    },
                    ThemeSubmenuItem {
                        key: "custom".to_string(),
                        label: self.localized_string_or("settings.option_custom", "Custom"),
                    },
                ]
            })
    }

    // Застосовує тему: тільки тут відкривається зовнішній JSON-файл теми.
    fn apply_external_theme_to_active_config(
        theme_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme_path = Self::theme_file_path_for_name(theme_name).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unsupported theme name: {theme_name}"),
            )
        })?;
        let external_theme = Self::read_external_theme_file(theme_path)?;
        let config_text =
            std::fs::read_to_string(Self::app_resource_path("src/interface/ui_config.json"))?;
        let mut config_json = serde_json::from_str::<serde_json::Value>(&config_text)?;

        let current = config_json.get_mut("current").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ui_config.json is missing current theme state",
            )
        })?;

        current["theme"] = serde_json::Value::String(external_theme.name);
        current["colors"] = Self::theme_colors_to_json(&external_theme.colors);

        Self::write_ui_config(&config_json)?;

        Ok(())
    }

    fn theme_file_path_for_name(theme_name: &str) -> Option<&'static str> {
        match theme_name {
            "light" => Some("src/interface/themes/light.json"),
            "dark" => Some("src/interface/themes/dark.json"),
            "custom" => Some("src/interface/themes/custom.json"),
            _ => None,
        }
    }

    // Застосовує мову: тільки тут відкривається зовнішній JSON-файл мови.
    fn apply_external_language_to_active_config(
        language_code: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let language_path = format!("src/interface/languages/{language_code}.json");
        let language_text = std::fs::read_to_string(Self::app_resource_path(&language_path))?;
        let language_json = serde_json::from_str::<serde_json::Value>(&language_text)?;
        let metadata = language_json.get("metadata").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "language file is missing metadata",
            )
        })?;

        let language = metadata
            .get("language_code")
            .and_then(|value| value.as_str())
            .unwrap_or(language_code)
            .to_string();
        let font_name = metadata
            .get("default_font")
            .and_then(|value| value.as_str())
            .unwrap_or("Times New Roman")
            .to_string();
        let font_size = metadata
            .get("default_font_size")
            .and_then(|value| value.as_u64())
            .unwrap_or(DEFAULT_UI_FONT_SIZE as u64);

        let mut strings = HashMap::new();
        Self::collect_localized_strings("", &language_json, &mut strings);

        let mut config_json = Self::read_ui_config()?;
        let current = config_json.get_mut("current").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ui_config.json is missing current state",
            )
        })?;

        current["language"] = serde_json::Value::String(language);
        current["font_name"] = serde_json::Value::String(font_name);
        current["font_size"] = serde_json::Value::Number(serde_json::Number::from(font_size));
        current["strings"] = serde_json::to_value(strings)?;

        Self::write_ui_config(&config_json)?;
        Ok(())
    }

    fn load_interface_settings() -> Result<InterfaceSettings, Box<dyn std::error::Error>> {
        let config_json = Self::read_ui_config()?;
        let current = config_json.get("current");
        let metadata = config_json.get("metadata");
        let strings = Self::strings_from_config(&config_json);

        let language = current
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                metadata
                    .and_then(|v| v.get("default_language"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("en")
            .to_string();

        let theme = current
            .and_then(|v| v.get("theme"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                metadata
                    .and_then(|v| v.get("default_theme"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("light")
            .to_string();

        let font_name = current
            .and_then(|v| v.get("font_name"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                metadata
                    .and_then(|v| v.get("default_font"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("Times New Roman")
            .to_string();

        let font_size = current
            .and_then(|v| v.get("font_size"))
            .and_then(|v| v.as_u64())
            .or_else(|| {
                metadata
                    .and_then(|v| v.get("default_font_size"))
                    .and_then(|v| v.as_u64())
            })
            .unwrap_or(DEFAULT_UI_FONT_SIZE as u64) as u32;

        Ok(InterfaceSettings {
            language,
            theme,
            font_name,
            font_size,
            strings,
        })
    }

    // Завантажує активну тему тільки з ui_config.json.
    // Якщо локальний конфіг недоступний або некоректний, повертається безпечна світла тема.
    fn load_active_theme_colors() -> UiTheme {
        match std::fs::read_to_string(Self::app_resource_path("src/interface/ui_config.json")) {
            Ok(config_text) => {
                match serde_json::from_str::<serde_json::Value>(&config_text) {
                    Ok(config_json) => {
                        if let Some(colors_obj) =
                            config_json.get("current").and_then(|c| c.get("colors"))
                        {
                            // Парсер кольорів лишається локальним для UI-шару.
                            let extract_color = |key: &str| -> UiColorRgba {
                                if let Some(color_obj) = colors_obj.get(key) {
                                    let r =
                                        color_obj.get("r").and_then(|v| v.as_f64()).unwrap_or(0.95)
                                            as f32;
                                    let g =
                                        color_obj.get("g").and_then(|v| v.as_f64()).unwrap_or(0.95)
                                            as f32;
                                    let b =
                                        color_obj.get("b").and_then(|v| v.as_f64()).unwrap_or(0.95)
                                            as f32;
                                    let a =
                                        color_obj.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0)
                                            as f32;
                                    UiColorRgba::new(r, g, b, a)
                                } else {
                                    UiTheme::default_light().colors.window_background
                                }
                            };

                            let active_theme_name = config_json
                                .get("current")
                                .and_then(|c| c.get("theme"))
                                .and_then(|theme| theme.as_str())
                                .unwrap_or("light")
                                .to_string();

                            return UiTheme {
                                name: active_theme_name,
                                colors: UiThemeColors {
                                    window_background: extract_color("window_background"),
                                    left_panel_background: extract_color("left_panel_background"),
                                    logs_background: extract_color("logs_background"),
                                    logs_text_color: extract_color("logs_text_color"),
                                    dialogue_background: extract_color("dialogue_background"),
                                    dialogue_text_color: extract_color("dialogue_text_color"),
                                    dialogue_user_nickname_color: extract_color(
                                        "dialogue_user_nickname_color",
                                    ),
                                    dialogue_lens_nickname_color: extract_color(
                                        "dialogue_lens_nickname_color",
                                    ),
                                    input_background: extract_color("input_background"),
                                    input_text_color: extract_color("input_text_color"),
                                    button_normal_background: extract_color(
                                        "button_normal_background",
                                    ),
                                    button_hover_background: extract_color(
                                        "button_hover_background",
                                    ),
                                    button_active_background: extract_color(
                                        "button_active_background",
                                    ),
                                    button_text_normal_color: extract_color(
                                        "button_text_normal_color",
                                    ),
                                    button_text_hover_color: extract_color(
                                        "button_text_hover_color",
                                    ),
                                    button_text_active_color: extract_color(
                                        "button_text_active_color",
                                    ),
                                    menu_background: extract_color("menu_background"),
                                    menu_hover_background: extract_color("menu_hover_background"),
                                    menu_border: extract_color("menu_border"),
                                    primary_text_color: extract_color("primary_text_color"),
                                    secondary_text_color: extract_color("secondary_text_color"),
                                    separator_color: extract_color("separator_color"),
                                    server_one_button_running_background: extract_color(
                                        "server_one_button_running_background",
                                    ),
                                    server_one_button_running_hover_background: extract_color(
                                        "server_one_button_running_hover_background",
                                    ),
                                    server_one_button_running_active_background: extract_color(
                                        "server_one_button_running_active_background",
                                    ),
                                    server_one_button_not_running_background: extract_color(
                                        "server_one_button_not_running_background",
                                    ),
                                    server_one_button_not_running_hover_background: extract_color(
                                        "server_one_button_not_running_hover_background",
                                    ),
                                    server_one_button_not_running_active_background: extract_color(
                                        "server_one_button_not_running_active_background",
                                    ),
                                    server_one_button_text_color: extract_color(
                                        "server_one_button_text_color",
                                    ),
                                    server_one_indicator_idle_fill: extract_color(
                                        "server_one_indicator_idle_fill",
                                    ),
                                    server_one_indicator_border: extract_color(
                                        "server_one_indicator_border",
                                    ),
                                },
                            };
                        }
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
        UiTheme::default_light()
    }

    fn estimate_text_width(text_value: &str, font_size: u16) -> f32 {
        text_value.chars().count() as f32 * font_size as f32 * 0.58
    }

    fn calculate_settings_button_metrics(&self, label: &str) -> SettingsButtonMetrics {
        let binding = self.current_menu_button_text_binding();
        let geometry =
            Self::calculate_button_geometry(label, binding, ButtonPlacement::Standalone, None);

        SettingsButtonMetrics {
            width: geometry.width,
            height: geometry.height,
        }
    }

    fn settings_first_menu_item_specs(&self) -> Vec<SettingsFirstMenuItem> {
        vec![
            SettingsFirstMenuItem {
                key: "language",
                label: self.localized_string_or("settings.section_language", "Language"),
            },
            SettingsFirstMenuItem {
                key: "theme",
                label: self.localized_string_or("settings.section_theme", "Theme"),
            },
            SettingsFirstMenuItem {
                key: "fonts",
                label: self.localized_string_or("settings.section_fonts", "Fonts"),
            },
            SettingsFirstMenuItem {
                key: "ai_models",
                label: self.localized_string_or("settings.section_ai_models", "AI Models"),
            },
            SettingsFirstMenuItem {
                key: "messages",
                label: self.localized_string_or("settings.section_messages", "Messages"),
            },
        ]
    }

    fn settings_first_menu_items(&self) -> Vec<String> {
        self.settings_first_menu_item_specs()
            .into_iter()
            .map(|item| item.label)
            .collect()
    }

    fn settings_first_menu_index(&self, key: &str) -> usize {
        self.settings_first_menu_item_specs()
            .into_iter()
            .position(|item| item.key == key)
            .unwrap_or(0)
    }

    fn theme_submenu_items(&self) -> Vec<ThemeSubmenuItem> {
        self.theme_items_from_config()
    }

    fn font_submenu_items(&self) -> Vec<FontSubmenuItem> {
        vec![
            FontSubmenuItem {
                key: "font.add".to_string(),
                label: self.localized_string_or("settings.option_font_add", "Add"),
            },
            FontSubmenuItem {
                key: "font.size".to_string(),
                label: self.localized_string_or("settings.label_font_size", "Size"),
            },
        ]
    }

    fn messages_submenu_items(&self) -> Vec<BasicSubmenuItem> {
        vec![BasicSubmenuItem {
            key: "messages.input".to_string(),
            label: self.localized_string_or("settings.option_input", "Input"),
        }]
    }

    fn ai_models_submenu_items(&self) -> Vec<BasicSubmenuItem> {
        vec![
            BasicSubmenuItem {
                key: "ai_models.reasoning".to_string(),
                label: self.localized_string_or("settings.option_reasoning_model", "Reasoning"),
            },
            BasicSubmenuItem {
                key: "ai_models.language".to_string(),
                label: self.localized_string_or("settings.option_language_model", "Language"),
            },
        ]
    }

    fn input_submenu_items(&self) -> Vec<BasicSubmenuItem> {
        vec![
            BasicSubmenuItem {
                key: "input.enter".to_string(),
                label: self.localized_string_or("settings.option_enter", "Enter"),
            },
            BasicSubmenuItem {
                key: "input.enter_ctrl".to_string(),
                label: self.localized_string_or("settings.option_enter_ctrl", "Enter+Ctrl"),
            },
        ]
    }

    fn debug_menu_items(&self) -> Vec<BasicSubmenuItem> {
        vec![
            BasicSubmenuItem {
                key: "debug.run_mock_reasoning_test".to_string(),
                label: self.localized_string_or(
                    "debug.run_mock_reasoning_test",
                    "Run Mock Reasoning Test",
                ),
            },
            BasicSubmenuItem {
                key: "debug.run_real_reasoning_test".to_string(),
                label: self.localized_string_or(
                    "debug.run_real_reasoning_test",
                    "Run Real Reasoning Test",
                ),
            },
            BasicSubmenuItem {
                key: "debug.show_raw_reasoning_result".to_string(),
                label: self.localized_string_or(
                    "debug.show_raw_reasoning_result",
                    "Show Raw Reasoning Result",
                ),
            },
            BasicSubmenuItem {
                key: "debug.show_reasoning_readiness".to_string(),
                label: self.localized_string_or(
                    "debug.show_reasoning_readiness",
                    "Show Reasoning Readiness",
                ),
            },
            BasicSubmenuItem {
                key: "debug.show_active_runtime_config".to_string(),
                label: self.localized_string_or(
                    "debug.show_active_runtime_config",
                    "Show Active Runtime Config",
                ),
            },
            BasicSubmenuItem {
                key: "debug.open_core_log".to_string(),
                label: self.localized_string_or("debug.open_core_log", "Open Core Log"),
            },
        ]
    }

    fn localized_string_or(&self, key: &str, fallback: &str) -> String {
        let value = self.interface.get_string(key);
        if value.trim().is_empty() || value == format!("[{}]", key) {
            fallback.to_string()
        } else {
            value
        }
    }

    fn current_language_text_binding(&self) -> LanguageTextBinding {
        let settings = self.interface.current_settings();
        LanguageTextBinding {
            font_name: settings.font_name.clone(),
            font_size: u16::try_from(settings.font_size).unwrap_or(DEFAULT_UI_FONT_SIZE),
        }
    }

    fn current_menu_button_text_binding(&self) -> MenuButtonTextBinding {
        self.current_language_text_binding()
            .menu_button_text_binding()
    }

    fn language_submenu_items(&self) -> Vec<LanguageSubmenuItem> {
        Self::language_metadata_from_config()
            .into_iter()
            .map(Self::language_submenu_item_from_metadata)
            .collect()
    }

    fn language_submenu_item_from_metadata(metadata: LanguageMetadata) -> LanguageSubmenuItem {
        LanguageSubmenuItem {
            key: metadata.code,
            label: metadata.language_name,
            font_name: metadata.default_font,
            font_size: metadata.default_font_size,
        }
    }

    fn language_metadata_for_code(language_code: &str) -> Option<LanguageMetadata> {
        Self::language_metadata_from_config()
            .into_iter()
            .find(|metadata| metadata.code == language_code)
    }

    fn nested_menu_button_height(&self) -> f32 {
        Self::calculate_menu_button_geometry("", self.current_menu_button_text_binding(), Some(0.0))
            .height
    }

    fn nested_menu_surface_padding() -> f32 {
        2.0
    }

    fn estimate_menu_button_text_width(label: &str, binding: MenuButtonTextBinding) -> f32 {
        match binding.font_name {
            Some("Times New Roman") => label
                .chars()
                .map(|character| {
                    let width_factor = if character.is_ascii() { 0.5 } else { 0.54 };
                    binding.font_size as f32 * width_factor
                })
                .sum::<f32>(),
            _ => Self::estimate_text_width(label, binding.font_size),
        }
    }

    fn calculate_button_text_height(binding: MenuButtonTextBinding) -> f32 {
        binding.font_size as f32 * 1.25
    }

    fn calculate_button_geometry(
        label: &str,
        binding: MenuButtonTextBinding,
        placement: ButtonPlacement,
        fixed_menu_width: Option<f32>,
    ) -> ButtonGeometry {
        let text_width = Self::estimate_menu_button_text_width(label, binding);
        let text_height = Self::calculate_button_text_height(binding);
        let standalone_width = text_width + BUTTON_HORIZONTAL_OFFSET * 2.0;
        let width = match placement {
            ButtonPlacement::Standalone => standalone_width,
            ButtonPlacement::MenuPanel => fixed_menu_width.unwrap_or(standalone_width),
        };
        let height = text_height + BUTTON_VERTICAL_OFFSET * 2.0;

        ButtonGeometry { width, height }
    }

    fn calculate_menu_button_geometry(
        label: &str,
        binding: MenuButtonTextBinding,
        fixed_menu_width: Option<f32>,
    ) -> ButtonGeometry {
        Self::calculate_button_geometry(
            label,
            binding,
            ButtonPlacement::MenuPanel,
            fixed_menu_width,
        )
    }

    fn calculate_menu_width_from_button_specs<I>(button_specs: I) -> f32
    where
        I: IntoIterator<Item = (String, MenuButtonTextBinding)>,
    {
        button_specs
            .into_iter()
            .map(|(label, binding)| {
                Self::calculate_menu_button_geometry(&label, binding, None).width
            })
            .fold(0.0, f32::max)
    }

    fn calculate_theme_submenu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.theme_submenu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_language_submenu_width(&self) -> f32 {
        Self::calculate_menu_width_from_button_specs(self.language_submenu_items().into_iter().map(
            |item| {
                let binding = Self::language_text_binding(&item);
                (item.label, binding.menu_button_text_binding())
            },
        ))
    }

    fn calculate_font_submenu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.font_submenu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_messages_submenu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.messages_submenu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_ai_models_submenu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.ai_models_submenu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_input_submenu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.input_submenu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_debug_menu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.debug_menu_items()
                .into_iter()
                .map(|item| (item.label, binding)),
        )
    }

    fn calculate_theme_submenu_offsets(&self, first_menu_width: f32) -> (f32, f32) {
        let theme_parent_index = self.settings_first_menu_index("theme");
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            first_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = Self::nested_menu_surface_padding()
            + theme_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_language_submenu_offsets(&self, first_menu_width: f32) -> (f32, f32) {
        let language_parent_index = self.settings_first_menu_index("language");
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            first_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = Self::nested_menu_surface_padding()
            + language_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_font_submenu_offsets(&self, first_menu_width: f32) -> (f32, f32) {
        let font_parent_index = self.settings_first_menu_index("fonts");
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            first_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = Self::nested_menu_surface_padding()
            + font_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_messages_submenu_offsets(&self, first_menu_width: f32) -> (f32, f32) {
        let messages_parent_index = self.settings_first_menu_index("messages");
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            first_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = Self::nested_menu_surface_padding()
            + messages_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_ai_models_submenu_offsets(&self, first_menu_width: f32) -> (f32, f32) {
        let ai_models_parent_index = self.settings_first_menu_index("ai_models");
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            first_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = Self::nested_menu_surface_padding()
            + ai_models_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_input_submenu_offsets(
        &self,
        messages_menu_width: f32,
        messages_menu_top_offset: f32,
    ) -> (f32, f32) {
        let input_parent_label = self.localized_string_or("settings.option_input", "Input");
        let input_parent_index = self
            .messages_submenu_items()
            .into_iter()
            .position(|item| item.label == input_parent_label)
            .unwrap_or(0);
        let menu_button_height = self.nested_menu_button_height();
        let parent_item_x = Self::nested_menu_surface_padding();
        let submenu_horizontal_overlap =
            messages_menu_width * (1.0 - MENU_HORIZONTAL_ATTACH_RATIO) - parent_item_x;
        let submenu_top_offset = messages_menu_top_offset
            + Self::nested_menu_surface_padding()
            + input_parent_index as f32 * (menu_button_height + 1.0)
            + menu_button_height * MENU_VERTICAL_ATTACH_RATIO;

        (submenu_horizontal_overlap, submenu_top_offset)
    }

    fn calculate_menu_width(&self) -> f32 {
        let binding = self.current_menu_button_text_binding();
        Self::calculate_menu_width_from_button_specs(
            self.settings_first_menu_items()
                .into_iter()
                .map(|item| (item, binding)),
        )
    }

    fn render_dialogue_history(
        &self,
        dialogue_text: Color,
        user_nickname_text: Color,
        lens_nickname_text: Color,
        text_binding: &LanguageTextBinding,
    ) -> Element<'_, Message> {
        let lines = self
            .state
            .dialogue_history
            .split('\n')
            .map(|line| {
                if let Some(rest) = line.strip_prefix("User:") {
                    row![
                        text("User:")
                            .size(text_binding.font_size)
                            .font(text_binding.font())
                            .style(iced::theme::Text::Color(user_nickname_text)),
                        text(rest.to_string())
                            .size(text_binding.font_size)
                            .font(text_binding.font())
                            .style(iced::theme::Text::Color(dialogue_text))
                            .width(Length::Fill)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
                } else if let Some(rest) = line.strip_prefix("LENS:") {
                    row![
                        text("LENS:")
                            .size(text_binding.font_size)
                            .font(text_binding.font())
                            .style(iced::theme::Text::Color(lens_nickname_text)),
                        text(rest.to_string())
                            .size(text_binding.font_size)
                            .font(text_binding.font())
                            .style(iced::theme::Text::Color(dialogue_text))
                            .width(Length::Fill)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
                } else {
                    text(line.to_string())
                        .size(text_binding.font_size)
                        .font(text_binding.font())
                        .style(iced::theme::Text::Color(dialogue_text))
                        .width(Length::Fill)
                        .into()
                }
            })
            .collect::<Vec<Element<Message>>>();

        column(lines).spacing(0).width(Length::Fill).into()
    }

    fn render_nested_menu_button<'a>(
        label: String,
        menu_width: f32,
        colors: &UiThemeColors,
        on_press: Option<Message>,
        visual_state: MenuButtonVisualState,
        text_binding: MenuButtonTextBinding,
    ) -> Element<'a, Message> {
        let button_style = ThemedMenuButtonStyle::from_colors_and_state(colors, visual_state);
        let press_message = on_press.unwrap_or(Message::MenuButtonNoop);
        let button_size =
            Self::calculate_menu_button_geometry(&label, text_binding, Some(menu_width));

        let mut label_text = text(label).size(text_binding.font_size);
        if let Some(font_name) = text_binding.font_name {
            label_text = label_text.font(Font::with_name(font_name));
        }

        let content: Element<'a, Message> = container(label_text)
            .padding(Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: BUTTON_HORIZONTAL_OFFSET,
            })
            .width(Length::Fixed(button_size.width))
            .height(Length::Fixed(button_size.height))
            .center_y()
            .into();

        button(content)
            .on_press(press_message)
            .width(Length::Fixed(button_size.width))
            .height(Length::Fixed(button_size.height))
            .padding(0)
            .style(iced::theme::Button::Custom(Box::new(button_style)))
            .into()
    }

    fn language_text_binding(item: &LanguageSubmenuItem) -> LanguageTextBinding {
        LanguageTextBinding {
            font_name: item.font_name.clone(),
            font_size: item.font_size,
        }
    }

    fn render_language_menu_button<'a>(
        item: LanguageSubmenuItem,
        menu_width: f32,
        colors: &UiThemeColors,
        visual_state: MenuButtonVisualState,
    ) -> Element<'a, Message> {
        let binding = Self::language_text_binding(&item);
        let language_key = item.key.clone();
        let button_style = ThemedMenuButtonStyle::from_colors_and_state(colors, visual_state);
        let button_size = Self::calculate_menu_button_geometry(
            &item.label,
            binding.menu_button_text_binding(),
            Some(menu_width),
        );
        let content: Element<'a, Message> = container(
            text(item.label)
                .size(binding.font_size)
                .font(binding.font()),
        )
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: BUTTON_HORIZONTAL_OFFSET,
        })
        .width(Length::Fixed(button_size.width))
        .height(Length::Fixed(button_size.height))
        .center_y()
        .into();

        button(content)
            .on_press(Message::SettingsMenuItemSelected(language_key))
            .width(Length::Fixed(button_size.width))
            .height(Length::Fixed(button_size.height))
            .padding(0)
            .style(iced::theme::Button::Custom(Box::new(button_style)))
            .into()
    }

    fn render_nested_menu_surface<'a>(
        buttons: Vec<Element<'a, Message>>,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let menu_col = column(buttons).spacing(1).width(Length::Fixed(menu_width));

        let menu_bg = Self::filled_menu_surface_background(colors);
        let menu_border = colors.menu_border;

        container(menu_col)
            .style(move |_theme: &Theme| Self::menu_surface_appearance(menu_bg, menu_border))
            .padding(2)
            .width(Length::Fixed(menu_width))
            .into()
    }

    fn filled_menu_surface_background(colors: &UiThemeColors) -> UiColorRgba {
        UiColorRgba {
            a: 1.0,
            ..colors.menu_background
        }
    }

    fn menu_surface_appearance(
        background: UiColorRgba,
        border: UiColorRgba,
    ) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Background::Color(background.to_iced_color())),
            border: iced::Border {
                width: 1.0,
                color: border.to_iced_color(),
                radius: 0.0.into(),
            },
            text_color: None,
            shadow: Default::default(),
        }
    }

    fn render_settings_overlay_menu(
        &self,
        level: SettingsOverlayMenuLevel,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Element<'_, Message> {
        let buttons = self.render_settings_overlay_menu_buttons(level, menu_width, colors);
        self.render_settings_overlay_menu_surface(level, buttons, menu_width, colors)
    }

    fn render_settings_overlay_menu_buttons(
        &self,
        level: SettingsOverlayMenuLevel,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Vec<Element<'_, Message>> {
        match level {
            SettingsOverlayMenuLevel::First => {
                let text_binding = self.current_menu_button_text_binding();

                self.settings_first_menu_item_specs()
                    .into_iter()
                    .map(|item| {
                        let on_press = Some(Message::MenuButtonNoop);
                        let visual_state = if item.key == "language"
                            && self.overlay_menus.settings_language_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if item.key == "theme"
                            && self.overlay_menus.settings_theme_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if item.key == "fonts"
                            && self.overlay_menus.settings_font_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if item.key == "ai_models"
                            && self.overlay_menus.settings_ai_models_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if item.key == "messages"
                            && self.overlay_menus.settings_messages_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if item.key == "language"
                            && self
                                .overlay_menus
                                .settings_language_submenu
                                .node
                                .parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else if item.key == "theme"
                            && self.overlay_menus.settings_theme_submenu.node.parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else if item.key == "fonts"
                            && self.overlay_menus.settings_font_submenu.node.parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else if item.key == "ai_models"
                            && self
                                .overlay_menus
                                .settings_ai_models_submenu
                                .node
                                .parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else if item.key == "messages"
                            && self
                                .overlay_menus
                                .settings_messages_submenu
                                .node
                                .parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else {
                            MenuButtonVisualState::Normal
                        };

                        let button = Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            on_press,
                            visual_state,
                            text_binding,
                        );

                        if item.key == "theme" {
                            mouse_area(button)
                                .on_enter(Message::SettingsThemeParentEntered)
                                .on_exit(Message::SettingsThemeParentExited)
                                .into()
                        } else if item.key == "language" {
                            mouse_area(button)
                                .on_enter(Message::SettingsLanguageParentEntered)
                                .on_exit(Message::SettingsLanguageParentExited)
                                .into()
                        } else if item.key == "fonts" {
                            mouse_area(button)
                                .on_enter(Message::SettingsFontParentEntered)
                                .on_exit(Message::SettingsFontParentExited)
                                .into()
                        } else if item.key == "ai_models" {
                            mouse_area(button)
                                .on_enter(Message::SettingsAiModelsParentEntered)
                                .on_exit(Message::SettingsAiModelsParentExited)
                                .into()
                        } else if item.key == "messages" {
                            mouse_area(button)
                                .on_enter(Message::SettingsMessagesParentEntered)
                                .on_exit(Message::SettingsMessagesParentExited)
                                .into()
                        } else {
                            button
                        }
                    })
                    .collect::<Vec<Element<Message>>>()
            }
            SettingsOverlayMenuLevel::ThemeSubmenu => {
                let text_binding = self.current_menu_button_text_binding();

                self.theme_submenu_items()
                    .into_iter()
                    .map(|item| {
                        let visual_state = MenuButtonVisualState::Normal;

                        Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            Some(Message::SettingsMenuItemSelected(item.key)),
                            visual_state,
                            text_binding,
                        )
                    })
                    .collect::<Vec<Element<Message>>>()
            }
            SettingsOverlayMenuLevel::LanguageSubmenu => self
                .language_submenu_items()
                .into_iter()
                .map(|item| {
                    let visual_state = MenuButtonVisualState::Normal;

                    Self::render_language_menu_button(item, menu_width, colors, visual_state)
                })
                .collect::<Vec<Element<Message>>>(),
            SettingsOverlayMenuLevel::FontSubmenu => {
                let text_binding = self.current_menu_button_text_binding();

                self.font_submenu_items()
                    .into_iter()
                    .map(|item| {
                        Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            Some(Message::SettingsMenuItemSelected(item.key)),
                            MenuButtonVisualState::Normal,
                            text_binding,
                        )
                    })
                    .collect::<Vec<Element<Message>>>()
            }
            SettingsOverlayMenuLevel::AiModelsSubmenu => {
                let text_binding = self.current_menu_button_text_binding();

                self.ai_models_submenu_items()
                    .into_iter()
                    .map(|item| {
                        Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            Some(Message::SettingsMenuItemSelected(item.key)),
                            MenuButtonVisualState::Normal,
                            text_binding,
                        )
                    })
                    .collect::<Vec<Element<Message>>>()
            }
            SettingsOverlayMenuLevel::MessagesSubmenu => {
                let input_parent_label = self.localized_string_or("settings.option_input", "Input");
                let text_binding = self.current_menu_button_text_binding();

                self.messages_submenu_items()
                    .into_iter()
                    .map(|item| {
                        let is_input_parent = item.label == input_parent_label;
                        let visual_state = if is_input_parent
                            && self.overlay_menus.settings_input_submenu.is_open()
                        {
                            MenuButtonVisualState::Active
                        } else if is_input_parent
                            && self.overlay_menus.settings_input_submenu.node.parent_active
                        {
                            MenuButtonVisualState::Hover
                        } else {
                            MenuButtonVisualState::Normal
                        };

                        let button = Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            Some(Message::MenuButtonNoop),
                            visual_state,
                            text_binding,
                        );

                        if is_input_parent {
                            mouse_area(button)
                                .on_enter(Message::SettingsInputParentEntered)
                                .on_exit(Message::SettingsInputParentExited)
                                .into()
                        } else {
                            button
                        }
                    })
                    .collect::<Vec<Element<Message>>>()
            }
            SettingsOverlayMenuLevel::InputSubmenu => {
                let text_binding = self.current_menu_button_text_binding();

                self.input_submenu_items()
                    .into_iter()
                    .map(|item| {
                        Self::render_nested_menu_button(
                            item.label,
                            menu_width,
                            colors,
                            Some(Message::SettingsMenuItemSelected(item.key)),
                            MenuButtonVisualState::Normal,
                            text_binding,
                        )
                    })
                    .collect::<Vec<Element<Message>>>()
            }
        }
    }

    fn render_settings_overlay_menu_surface<'a>(
        &self,
        level: SettingsOverlayMenuLevel,
        buttons: Vec<Element<'a, Message>>,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let surface = Self::render_nested_menu_surface(buttons, menu_width, colors);
        let surface = Self::render_visible_menu_surface(surface, menu_width, colors);

        match level {
            SettingsOverlayMenuLevel::First => mouse_area(surface)
                .on_enter(Message::SettingsMenuEntered)
                .on_exit(Message::SettingsMenuExited)
                .into(),
            SettingsOverlayMenuLevel::ThemeSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsThemeSubmenuEntered)
                .on_exit(Message::SettingsThemeSubmenuExited)
                .into(),
            SettingsOverlayMenuLevel::LanguageSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsLanguageSubmenuEntered)
                .on_exit(Message::SettingsLanguageSubmenuExited)
                .into(),
            SettingsOverlayMenuLevel::FontSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsFontSubmenuEntered)
                .on_exit(Message::SettingsFontSubmenuExited)
                .into(),
            SettingsOverlayMenuLevel::AiModelsSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsAiModelsSubmenuEntered)
                .on_exit(Message::SettingsAiModelsSubmenuExited)
                .into(),
            SettingsOverlayMenuLevel::MessagesSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsMessagesSubmenuEntered)
                .on_exit(Message::SettingsMessagesSubmenuExited)
                .into(),
            SettingsOverlayMenuLevel::InputSubmenu => mouse_area(surface)
                .on_enter(Message::SettingsInputSubmenuEntered)
                .on_exit(Message::SettingsInputSubmenuExited)
                .into(),
        }
    }

    fn render_visible_menu_surface<'a>(
        surface: Element<'a, Message>,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let menu_bg = Self::filled_menu_surface_background(colors);
        let menu_border = colors.menu_border;

        container(surface)
            .style(move |_theme: &Theme| Self::menu_surface_appearance(menu_bg, menu_border))
            .width(Length::Fixed(menu_width))
            .into()
    }

    fn render_base_layout(
        &self,
        colors: &UiThemeColors,
        settings_title_text: String,
        settings_button_text: String,
        debug_button_text: String,
        logs_title_text: String,
        dialogue_placeholder: String,
        send_button_text: String,
    ) -> (
        Element<'_, Message>,
        SettingsButtonMetrics,
        SettingsButtonMetrics,
    ) {
        // Базовий макет містить лише елементи першої версії оболонки.
        let primary_text = colors.primary_text_color.to_iced_color();
        let logs_bg = colors.logs_background.to_iced_color();
        let logs_text = colors.logs_text_color.to_iced_color();
        let left_panel_bg = colors.left_panel_background.to_iced_color();
        let control_separator_bg = colors.separator_color.to_iced_color();
        let button_style = ThemedButtonStyle::from_colors(colors);
        let text_input_style = ThemedTextInputStyle::from_colors(colors);
        let text_binding = self.current_language_text_binding();

        let settings_title = text(settings_title_text)
            .size(text_binding.font_size)
            .font(text_binding.font())
            .style(iced::theme::Text::Color(primary_text))
            .width(Length::Fill);
        let settings_metrics = self.calculate_settings_button_metrics(&settings_button_text);
        let debug_metrics = self.calculate_settings_button_metrics(&debug_button_text);
        let server_one_button_text = self.localized_string_or("ui.button_server_one", "Сервер 1");
        let server_one_metrics = self.calculate_settings_button_metrics(&server_one_button_text);

        let settings_button = button(
            container(
                text(settings_button_text)
                    .size(text_binding.font_size)
                    .font(text_binding.font())
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y(),
        )
        .on_press(Message::SettingsPressed)
        .style(iced::theme::Button::Custom(Box::new(button_style)))
        .padding(0)
        .width(Length::Fixed(settings_metrics.width))
        .height(Length::Fixed(settings_metrics.height));

        let settings_button = mouse_area(settings_button)
            .on_enter(Message::SettingsParentEntered)
            .on_exit(Message::SettingsParentExited);

        let debug_button = button(
            container(
                text(debug_button_text)
                    .size(text_binding.font_size)
                    .font(text_binding.font())
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y(),
        )
        .on_press(Message::DebugPressed)
        .style(iced::theme::Button::Custom(Box::new(button_style)))
        .padding(0)
        .width(Length::Fixed(debug_metrics.width))
        .height(Length::Fixed(debug_metrics.height));

        let debug_button = mouse_area(debug_button)
            .on_enter(Message::DebugParentEntered)
            .on_exit(Message::DebugParentExited);

        let server_one_button = button(
            container(
                text(server_one_button_text)
                    .size(text_binding.font_size)
                    .font(text_binding.font())
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y(),
        )
        .on_press(Message::ServerOnePressed)
        .style(iced::theme::Button::Custom(Box::new(
            ServerControlButtonStyle::new(self.server_one_status, colors),
        )))
        .padding(0)
        .width(Length::Fixed(server_one_metrics.width))
        .height(Length::Fixed(server_one_metrics.height));

        let server_one_model_led_fill =
            Self::model_indicator_color(self.reasoning_model_indicator_state, colors);
        let server_one_led_fill = colors.server_one_indicator_idle_fill.to_iced_color();
        let server_one_led_border = colors.server_one_indicator_border.to_iced_color();
        let server_one_led_one = container(text(""))
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .style(move |_theme: &Theme| iced::widget::container::Appearance {
                background: Some(Background::Color(server_one_model_led_fill)),
                border: iced::Border {
                    width: 1.0,
                    color: server_one_led_border,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });
        let server_one_led_two = container(text(""))
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .style(move |_theme: &Theme| iced::widget::container::Appearance {
                background: Some(Background::Color(server_one_led_fill)),
                border: iced::Border {
                    width: 1.0,
                    color: server_one_led_border,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });
        let server_one_leds = container(row![server_one_led_one, server_one_led_two].spacing(8))
            .width(Length::Fixed(server_one_metrics.width));

        let control_separator = container(text(""))
            .height(Length::Fixed(1.0))
            .width(Length::Fill)
            .style(move |_theme: &Theme| iced::widget::container::Appearance {
                background: Some(Background::Color(control_separator_bg)),
                ..Default::default()
            });
        let control_separator = container(control_separator).padding([
            CONTROL_SEPARATOR_VERTICAL_PADDING,
            0.0,
            CONTROL_SEPARATOR_VERTICAL_PADDING,
            0.0,
        ]);

        let settings_controls: Element<Message> = column![
            server_one_leds,
            server_one_button,
            control_separator,
            settings_button,
            debug_button
        ]
        .spacing(5)
        .width(Length::Fill)
        .into();

        let settings_panel = container(
            column![
                settings_title,
                container(settings_controls).padding(10).width(Length::Fill)
            ]
            .spacing(5)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .style(move |_theme: &Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(left_panel_bg)),
            ..Default::default()
        })
        .padding(5)
        .height(Length::FillPortion(2))
        .width(Length::Fill);

        let logs_title = text(logs_title_text)
            .size(text_binding.font_size)
            .font(text_binding.font())
            .style(iced::theme::Text::Color(primary_text))
            .width(Length::Fill);
        let logs_content = scrollable(
            text(&self.state.technical_output)
                .size(text_binding.font_size)
                .font(text_binding.font())
                .style(iced::theme::Text::Color(logs_text))
                .width(Length::Fill),
        )
        .height(Length::Fill);

        let logs_panel = container(
            column![logs_title, logs_content]
                .spacing(5)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_theme: &Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(logs_bg)),
            ..Default::default()
        })
        .padding(5)
        .height(Length::FillPortion(1))
        .width(Length::Fill);

        let left_column = column![settings_panel, logs_panel]
            .spacing(10)
            .width(Length::Fixed(250.0))
            .height(Length::Fill);

        let dialogue_bg_color = colors.dialogue_background.to_iced_color();
        let dialogue_text = colors.dialogue_text_color.to_iced_color();
        let dialogue_user_nickname_text = colors.dialogue_user_nickname_color.to_iced_color();
        let dialogue_lens_nickname_text = colors.dialogue_lens_nickname_color.to_iced_color();
        let input_bg = colors.input_background.to_iced_color();
        let window_bg = colors.window_background.to_iced_color();

        let dialogue_area = container(
            scrollable(self.render_dialogue_history(
                dialogue_text,
                dialogue_user_nickname_text,
                dialogue_lens_nickname_text,
                &text_binding,
            ))
            .id(self.dialogue_scroll_id.clone())
            .height(Length::Fill),
        )
        .style(move |_theme: &Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(dialogue_bg_color)),
            ..Default::default()
        })
        .padding(10)
        .height(Length::Fill)
        .width(Length::Fill);

        let send_button_geometry = Self::calculate_button_geometry(
            &send_button_text,
            text_binding.menu_button_text_binding(),
            ButtonPlacement::Standalone,
            None,
        );
        let input_editor_height = (text_binding.font_size as f32 * 1.3 * INPUT_VISIBLE_LINES)
            + INPUT_VERTICAL_PADDING * 2.0;
        let input_control_height = send_button_geometry.height.max(input_editor_height);

        let input_field = text_editor(&self.input_content)
            .on_action(Message::InputEdited)
            .font(text_binding.font())
            .highlight::<InputTextHighlighter>(colors.input_text_color, input_text_format)
            .padding(Padding {
                top: INPUT_VERTICAL_PADDING,
                right: INPUT_HORIZONTAL_PADDING,
                bottom: INPUT_VERTICAL_PADDING,
                left: INPUT_HORIZONTAL_PADDING,
            })
            .height(Length::Shrink)
            .style(iced::theme::TextEditor::Custom(Box::new(text_input_style)));

        let input_field_scroll = scrollable(input_field)
            .id(self.input_scroll_id.clone())
            .height(Length::Fixed(input_control_height))
            .width(Length::Fill);

        let placeholder_color = colors.secondary_text_color.to_iced_color();
        let placeholder_text = if self.state.input.is_empty() {
            dialogue_placeholder
        } else {
            String::new()
        };
        let placeholder_layer = container(
            text(placeholder_text)
                .size(text_binding.font_size)
                .font(text_binding.font())
                .style(iced::theme::Text::Color(placeholder_color)),
        )
        .padding(Padding {
            top: INPUT_VERTICAL_PADDING,
            right: INPUT_HORIZONTAL_PADDING,
            bottom: INPUT_VERTICAL_PADDING,
            left: INPUT_HORIZONTAL_PADDING,
        })
        .height(Length::Fixed(input_control_height))
        .width(Length::Fill);

        let input_field: Element<Message> = column![placeholder_layer, input_field_scroll]
            .spacing(-input_control_height)
            .height(Length::Fixed(input_control_height))
            .width(Length::Fill)
            .into();

        let input_field = container(input_field)
            .height(Length::Fixed(input_control_height))
            .width(Length::Fill)
            .style(move |_theme: &Theme| iced::widget::container::Appearance {
                background: Some(Background::Color(input_bg)),
                ..Default::default()
            });

        let send_button_text_color = colors.button_text_normal_color.to_iced_color();
        let send_button = button(
            container(
                text(send_button_text)
                    .size(text_binding.font_size)
                    .font(text_binding.font())
                    .style(iced::theme::Text::Color(send_button_text_color))
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            )
            .width(Length::Fixed(send_button_geometry.width))
            .height(Length::Fixed(send_button_geometry.height))
            .center_x()
            .center_y(),
        )
        .on_press(Message::SendPressed)
        .style(iced::theme::Button::Custom(Box::new(button_style)))
        .height(Length::Fixed(input_control_height))
        .width(Length::Fixed(send_button_geometry.width))
        .padding(0);

        let input_row = container(
            row![input_field, send_button]
                .spacing(10)
                .width(Length::Fill),
        )
        .padding([20, 0, 0, 0])
        .width(Length::Fill)
        .style(move |_theme: &Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(dialogue_bg_color)),
            ..Default::default()
        });

        let right_column = container(
            column![dialogue_area, input_row]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_theme: &Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(window_bg)),
            ..Default::default()
        })
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill);

        let main_ui: Element<Message> = row![left_column, right_column]
            .spacing(10)
            .padding(10)
            .height(Length::Fill)
            .into();

        (main_ui, settings_metrics, debug_metrics)
    }

    fn render_overlay_layer<'a>(
        &'a self,
        settings_metrics: SettingsButtonMetrics,
        debug_metrics: SettingsButtonMetrics,
        colors: &UiThemeColors,
    ) -> Option<Element<'a, Message>> {
        if let Some(scene) = self.settings_overlay_scene() {
            Some(self.render_settings_overlay_scene(scene, settings_metrics, colors))
        } else if self.overlay_menus.debug_menu.is_open() {
            Some(self.render_debug_overlay_scene(settings_metrics, debug_metrics, colors))
        } else {
            None
        }
    }

    fn settings_overlay_scene(&self) -> Option<SettingsOverlayScene> {
        if !self.overlay_menus.settings_first_menu.is_open() {
            return None;
        }

        let first_menu_width = self.calculate_menu_width();
        let mut child_nested_menu = None;
        let nested_menu = if self.overlay_menus.settings_theme_submenu.is_open() {
            let width = self.calculate_theme_submenu_width();
            let (horizontal_overlap, top_offset) =
                self.calculate_theme_submenu_offsets(first_menu_width);

            Some(SettingsOverlayNestedMenuScene {
                level: SettingsOverlayMenuLevel::ThemeSubmenu,
                width,
                horizontal_overlap,
                top_offset,
            })
        } else if self.overlay_menus.settings_language_submenu.is_open() {
            let width = self.calculate_language_submenu_width();
            let (horizontal_overlap, top_offset) =
                self.calculate_language_submenu_offsets(first_menu_width);

            Some(SettingsOverlayNestedMenuScene {
                level: SettingsOverlayMenuLevel::LanguageSubmenu,
                width,
                horizontal_overlap,
                top_offset,
            })
        } else if self.overlay_menus.settings_font_submenu.is_open() {
            let width = self.calculate_font_submenu_width();
            let (horizontal_overlap, top_offset) =
                self.calculate_font_submenu_offsets(first_menu_width);

            Some(SettingsOverlayNestedMenuScene {
                level: SettingsOverlayMenuLevel::FontSubmenu,
                width,
                horizontal_overlap,
                top_offset,
            })
        } else if self.overlay_menus.settings_ai_models_submenu.is_open() {
            let width = self.calculate_ai_models_submenu_width();
            let (horizontal_overlap, top_offset) =
                self.calculate_ai_models_submenu_offsets(first_menu_width);

            Some(SettingsOverlayNestedMenuScene {
                level: SettingsOverlayMenuLevel::AiModelsSubmenu,
                width,
                horizontal_overlap,
                top_offset,
            })
        } else if self.overlay_menus.settings_messages_submenu.is_open() {
            let width = self.calculate_messages_submenu_width();
            let (horizontal_overlap, top_offset) =
                self.calculate_messages_submenu_offsets(first_menu_width);

            if self.overlay_menus.settings_input_submenu.is_open() {
                let input_width = self.calculate_input_submenu_width();
                let (input_horizontal_overlap, input_top_offset) =
                    self.calculate_input_submenu_offsets(width, top_offset);
                child_nested_menu = Some(SettingsOverlayNestedMenuScene {
                    level: SettingsOverlayMenuLevel::InputSubmenu,
                    width: input_width,
                    horizontal_overlap: input_horizontal_overlap,
                    top_offset: input_top_offset,
                });
            }

            Some(SettingsOverlayNestedMenuScene {
                level: SettingsOverlayMenuLevel::MessagesSubmenu,
                width,
                horizontal_overlap,
                top_offset,
            })
        } else {
            None
        };

        Some(SettingsOverlayScene {
            first_menu_width,
            nested_menu,
            child_nested_menu,
        })
    }

    fn render_settings_overlay_scene<'a>(
        &'a self,
        scene: SettingsOverlayScene,
        settings_metrics: SettingsButtonMetrics,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let first_menu = self.render_settings_overlay_menu(
            SettingsOverlayMenuLevel::First,
            scene.first_menu_width,
            colors,
        );
        let menu_branch = self.render_settings_overlay_branch(first_menu, scene, colors);
        let horizontal_offset = settings_metrics.width * MENU_HORIZONTAL_ATTACH_RATIO;
        let overlap_y = settings_metrics.height * (1.0 - MENU_VERTICAL_ATTACH_RATIO);

        container(menu_branch)
            .padding([overlap_y, 0.0, 0.0, horizontal_offset])
            .width(Length::Shrink)
            .into()
    }

    fn render_settings_overlay_branch<'a>(
        &'a self,
        first_menu: Element<'a, Message>,
        scene: SettingsOverlayScene,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        if let Some(nested_menu_scene) = scene.nested_menu {
            let nested_menu = self.render_settings_overlay_nested_menu(nested_menu_scene, colors);

            if let Some(child_nested_menu_scene) = scene.child_nested_menu {
                let child_nested_menu =
                    self.render_settings_overlay_nested_menu(child_nested_menu_scene, colors);
                let nested_branch = row![nested_menu, child_nested_menu]
                    .spacing(-child_nested_menu_scene.horizontal_overlap);

                row![first_menu, nested_branch]
                    .spacing(-nested_menu_scene.horizontal_overlap)
                    .into()
            } else {
                row![first_menu, nested_menu]
                    .spacing(-nested_menu_scene.horizontal_overlap)
                    .into()
            }
        } else {
            first_menu
        }
    }

    fn render_debug_overlay_scene<'a>(
        &'a self,
        settings_metrics: SettingsButtonMetrics,
        debug_metrics: SettingsButtonMetrics,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let menu_width = self.calculate_debug_menu_width();
        let debug_menu = self.render_debug_overlay_menu(menu_width, colors);
        let horizontal_offset = debug_metrics.width * MENU_HORIZONTAL_ATTACH_RATIO;
        let top_offset = settings_metrics.height
            + 5.0
            + debug_metrics.height * (1.0 - MENU_VERTICAL_ATTACH_RATIO);

        container(debug_menu)
            .padding([top_offset, 0.0, 0.0, horizontal_offset])
            .width(Length::Shrink)
            .into()
    }

    fn render_debug_overlay_menu<'a>(
        &'a self,
        menu_width: f32,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        let text_binding = self.current_menu_button_text_binding();
        let buttons = self
            .debug_menu_items()
            .into_iter()
            .map(|item| {
                Self::render_nested_menu_button(
                    item.label,
                    menu_width,
                    colors,
                    Some(Message::DebugMenuItemSelected(item.key)),
                    MenuButtonVisualState::Normal,
                    text_binding,
                )
            })
            .collect::<Vec<Element<Message>>>();
        let surface = Self::render_nested_menu_surface(buttons, menu_width, colors);
        let surface = Self::render_visible_menu_surface(surface, menu_width, colors);

        mouse_area(surface)
            .on_enter(Message::DebugMenuEntered)
            .on_exit(Message::DebugMenuExited)
            .into()
    }

    fn render_settings_overlay_nested_menu<'a>(
        &'a self,
        scene: SettingsOverlayNestedMenuScene,
        colors: &UiThemeColors,
    ) -> Element<'a, Message> {
        container(self.render_settings_overlay_menu(scene.level, scene.width, colors))
            .padding([scene.top_offset, 0.0, 0.0, 0.0])
            .width(Length::Shrink)
            .into()
    }

    fn render_scene_layers(
        &self,
        colors: &UiThemeColors,
        settings_title_text: String,
        settings_button_text: String,
        debug_button_text: String,
        logs_title_text: String,
        dialogue_placeholder: String,
        send_button_text: String,
    ) -> Element<'_, Message> {
        let (main_ui, settings_metrics, debug_metrics) = self.render_base_layout(
            colors,
            settings_title_text,
            settings_button_text,
            debug_button_text,
            logs_title_text,
            dialogue_placeholder,
            send_button_text,
        );
        let settings_menu_overlay =
            self.render_overlay_layer(settings_metrics, debug_metrics, colors);

        let root_content: Element<Message> =
            if let Some(settings_menu_overlay) = settings_menu_overlay {
                let root_overlay_top = 10.0 + 5.0 + 14.0 * 1.25 + 5.0 + 10.0;
                let root_overlay_left = 10.0 + 5.0 + 10.0;
                let root_overlay_layer_height = iced::window::Settings::default().size.height;

                column![
                    main_ui,
                    container(settings_menu_overlay)
                        .padding([root_overlay_top, 0.0, 0.0, root_overlay_left])
                        .height(Length::Fixed(root_overlay_layer_height))
                        .width(Length::Shrink)
                ]
                .spacing(-root_overlay_layer_height)
                .into()
            } else {
                main_ui
            };

        container(root_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl Application for App {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let interface_settings =
            Self::load_interface_settings().unwrap_or_else(|_| InterfaceSettings {
                language: "en".to_string(),
                theme: "light".to_string(),
                font_name: "Times New Roman".to_string(),
                font_size: DEFAULT_UI_FONT_SIZE as u32,
                strings: HashMap::new(),
            });

        let interface = InterfaceManager::new(interface_settings);

        let mut app = Self {
            state: State::new(),
            logger: Logger::new(),
            interface,
            overlay_menus: MenuOverlayState::closed(),
            input_content: text_editor::Content::new(),
            dialogue_scroll_id: iced::widget::scrollable::Id::new("main-dialogue-scroll"),
            input_scroll_id: iced::widget::scrollable::Id::new("dialogue-input-scroll"),
            skip_next_editor_enter: false,
            skip_next_editor_paste: false,
            submit_shortcut: InputSubmitShortcut::Enter,
            server_one_status: ServerOneStatus::NotRunning,
            reasoning_model_indicator_state: ModelIndicatorState::Empty,
        };

        app.logger.log_info("Application initialized");
        app.refresh_server_one_status_for_button();
        app.refresh_reasoning_model_indicator_state();
        // Старт застосунку виконує той самий сценарій, що й кнопка запуску.
        orchestrator::launch_startup_test(&mut app.state, &mut app.logger);
        app.logger
            .log_info("Displaying response from startup launch");
        app.state.update_technical_output(app.logger.get_logs());

        (app, Command::none())
    }

    fn title(&self) -> String {
        "LENS Desktop Shell v0".to_string()
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _status| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if is_paste_shortcut(key.as_ref(), modifiers) =>
            {
                Some(Message::KeyboardPastePressed)
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if matches!(
                    key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Enter)
                ) && modifiers.control() =>
            {
                Some(Message::KeyboardEnterPressed {
                    control: modifiers.control(),
                })
            }
            _ => None,
        })
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        // Тут обробляються лише UI-події та прості переходи стану оболонки.
        match message {
            Message::InputEdited(action) => {
                let should_follow_input = matches!(action, text_editor::Action::Edit(_));

                if self.skip_next_editor_enter
                    && matches!(action, text_editor::Action::Edit(text_editor::Edit::Enter))
                {
                    self.skip_next_editor_enter = false;
                    return Command::none();
                }

                if self.skip_next_editor_paste
                    && matches!(
                        action,
                        text_editor::Action::Edit(text_editor::Edit::Paste(_))
                    )
                {
                    self.skip_next_editor_paste = false;
                    return Command::none();
                }

                if self.submit_shortcut == InputSubmitShortcut::Enter
                    && matches!(action, text_editor::Action::Edit(text_editor::Edit::Enter))
                {
                    self.submit_current_input(self.submit_shortcut.label());
                    return self.scroll_dialogue_to_bottom();
                }

                if self.state.app_state == AppState::ShowingResponse {
                    self.state.set_state(AppState::Ready);
                    self.logger
                        .log_action("User started typing - transitioned to Ready state");
                }
                self.input_content.perform(action);
                self.state.update_input(self.input_content_text());
                self.state.update_technical_output(self.logger.get_logs());

                if should_follow_input {
                    return self.scroll_input_to_bottom();
                }
            }
            Message::SendPressed => {
                self.submit_current_input("send button");
                return self.scroll_dialogue_to_bottom();
            }
            Message::KeyboardEnterPressed { control } => {
                if self.submit_shortcut.matches_enter(control) {
                    self.skip_next_editor_enter = true;
                    self.submit_current_input(self.submit_shortcut.label());
                    return self.scroll_dialogue_to_bottom();
                }
            }
            Message::KeyboardPastePressed => {
                self.skip_next_editor_paste = true;
                return iced::clipboard::read(Message::ClipboardTextRead);
            }
            Message::ClipboardTextRead(clipboard_text) => {
                if let Some(text) = clipboard_text {
                    self.paste_plain_text_into_input(text);
                    return self.scroll_input_to_bottom();
                }

                self.skip_next_editor_paste = false;
            }
            Message::SettingsPressed => {
                let was_open = self.overlay_menus.settings_first_menu.is_open();
                self.overlay_menus.debug_menu.close_now();
                self.overlay_menus.settings_first_menu.toggle_open();
                if was_open && !self.overlay_menus.settings_first_menu.is_open() {
                    self.overlay_menus.settings_theme_submenu.close_now();
                    self.overlay_menus.settings_language_submenu.close_now();
                    self.overlay_menus.settings_font_submenu.close_now();
                    self.overlay_menus.settings_ai_models_submenu.close_now();
                    self.overlay_menus.settings_messages_submenu.close_now();
                    self.overlay_menus.settings_input_submenu.close_now();
                }
                self.logger
                    .log_action("Settings button pressed - toggling menu");
                self.state.update_technical_output(self.logger.get_logs());
            }
            Message::DebugPressed => {
                self.overlay_menus.close_settings_branch();
                self.overlay_menus.debug_menu.toggle_open();
                self.logger
                    .log_action("Debug button pressed - toggling menu");
                self.state.update_technical_output(self.logger.get_logs());
            }
            Message::ServerOnePressed => {
                self.handle_server_one_pressed();
                return self.scroll_dialogue_to_bottom();
            }
            Message::MenuButtonNoop => {}
            Message::SettingsMenuItemSelected(item) => {
                self.logger
                    .log_action(&format!("Settings menu item selected: {}", item));
                if item == "input.enter" {
                    self.submit_shortcut = InputSubmitShortcut::Enter;
                    self.logger
                        .log_info("Input message send shortcut selected: Enter");
                } else if item == "input.enter_ctrl" {
                    self.submit_shortcut = InputSubmitShortcut::EnterCtrl;
                    self.logger
                        .log_info("Input message send shortcut selected: Enter+Ctrl");
                } else if let Some(language_metadata) = Self::language_metadata_for_code(&item) {
                    match Self::apply_external_language_to_active_config(&language_metadata.code)
                        .and_then(|_| Self::load_interface_settings())
                    {
                        Ok(settings) => {
                            self.interface.apply_settings(settings);
                            self.logger.log_info(&format!(
                                "Language applied from settings menu: {}",
                                item
                            ));
                        }
                        Err(error) => self.logger.log_error(&format!(
                            "Failed to apply language from settings menu: {}",
                            error
                        )),
                    }
                } else if Self::theme_file_path_for_name(&item).is_some() {
                    match Self::apply_external_theme_to_active_config(&item)
                        .and_then(|_| Self::load_interface_settings())
                    {
                        Ok(settings) => {
                            self.interface.apply_settings(settings);
                            self.logger
                                .log_info(&format!("Theme applied from settings menu: {}", item));
                        }
                        Err(error) => self.logger.log_error(&format!(
                            "Failed to apply theme from settings menu: {}",
                            error
                        )),
                    }
                }
                self.overlay_menus.settings_first_menu.close_now();
                self.overlay_menus.settings_theme_submenu.close_now();
                self.overlay_menus.settings_language_submenu.close_now();
                self.overlay_menus.settings_font_submenu.close_now();
                self.overlay_menus.settings_ai_models_submenu.close_now();
                self.overlay_menus.settings_messages_submenu.close_now();
                self.overlay_menus.settings_input_submenu.close_now();
                self.state.update_technical_output(self.logger.get_logs());
            }
            Message::DebugMenuItemSelected(item) => {
                self.logger
                    .log_action(&format!("Debug menu item selected: {}", item));
                self.overlay_menus.debug_menu.close_now();
                if item == "debug.run_mock_reasoning_test" {
                    self.run_mock_reasoning_test();
                    return self.scroll_dialogue_to_bottom();
                }
                if item == "debug.show_reasoning_readiness" {
                    self.show_reasoning_readiness();
                    return self.scroll_dialogue_to_bottom();
                }
                if item == "debug.show_active_runtime_config" {
                    self.show_active_runtime_config();
                    return self.scroll_dialogue_to_bottom();
                }
                if item == "debug.run_real_reasoning_test" {
                    self.run_manual_real_reasoning_test();
                    return self.scroll_dialogue_to_bottom();
                }
                if item == "debug.show_raw_reasoning_result" {
                    self.show_raw_reasoning_result();
                    return self.scroll_dialogue_to_bottom();
                }
                if item == "debug.open_core_log" {
                    self.open_core_log();
                    return self.scroll_dialogue_to_bottom();
                }
                self.state.update_technical_output(self.logger.get_logs());
            }
            Message::SettingsParentEntered => {
                self.overlay_menus.debug_menu.close_now();
                self.overlay_menus
                    .settings_first_menu
                    .open_from_parent_hover(Instant::now());
            }
            Message::SettingsParentExited => {
                self.overlay_menus
                    .set_settings_parent_button_active(false, Instant::now());
                if let Some(command) =
                    self.settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                {
                    return command;
                }
            }
            Message::SettingsMenuEntered => {
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsMenuExited => {
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) =
                    self.settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                {
                    return command;
                }
            }
            Message::SettingsCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::First,
                    now,
                ) {
                    self.overlay_menus.settings_theme_submenu.close_now();
                    self.overlay_menus.settings_language_submenu.close_now();
                    self.overlay_menus.settings_font_submenu.close_now();
                    self.overlay_menus.settings_ai_models_submenu.close_now();
                    self.overlay_menus.settings_messages_submenu.close_now();
                    self.overlay_menus.settings_input_submenu.close_now();
                    self.state.update_technical_output(self.logger.get_logs());
                }
            }
            Message::DebugParentEntered => {
                self.overlay_menus.close_settings_branch();
                self.overlay_menus
                    .debug_menu
                    .open_from_parent_hover(Instant::now());
            }
            Message::DebugParentExited => {
                self.overlay_menus
                    .debug_menu
                    .set_parent_button_active(false, Instant::now());
                if self.overlay_menus.debug_menu.has_pending_close() {
                    return Self::debug_menu_close_delay_command();
                }
            }
            Message::DebugMenuEntered => {
                self.overlay_menus
                    .debug_menu
                    .set_menu_zone_active(true, Instant::now());
            }
            Message::DebugMenuExited => {
                self.overlay_menus
                    .debug_menu
                    .set_menu_zone_active(false, Instant::now());
                if self.overlay_menus.debug_menu.has_pending_close() {
                    return Self::debug_menu_close_delay_command();
                }
            }
            Message::DebugCloseDelayTick(now) => {
                if self
                    .overlay_menus
                    .debug_menu
                    .close_if_pending_expired(now, Duration::from_millis(DEBUG_MENU_CLOSE_DELAY_MS))
                {
                    self.state.update_technical_output(self.logger.get_logs());
                }
            }
            Message::SettingsThemeParentEntered => {
                self.overlay_menus.settings_language_submenu.close_now();
                self.overlay_menus.settings_font_submenu.close_now();
                self.overlay_menus.settings_ai_models_submenu.close_now();
                self.overlay_menus.settings_messages_submenu.close_now();
                self.overlay_menus.settings_input_submenu.close_now();
                self.overlay_menus
                    .settings_theme_submenu
                    .set_parent_item_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsThemeParentExited => {
                self.overlay_menus
                    .settings_theme_submenu
                    .set_parent_item_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::ThemeSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsThemeSubmenuEntered => {
                self.overlay_menus
                    .settings_theme_submenu
                    .set_submenu_zone_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsThemeSubmenuExited => {
                self.overlay_menus
                    .settings_theme_submenu
                    .set_submenu_zone_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::ThemeSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsThemeCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::ThemeSubmenu,
                    now,
                ) {
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self
                        .settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                    {
                        return command;
                    }
                }
            }
            Message::SettingsLanguageParentEntered => {
                self.overlay_menus.settings_theme_submenu.close_now();
                self.overlay_menus.settings_font_submenu.close_now();
                self.overlay_menus.settings_ai_models_submenu.close_now();
                self.overlay_menus.settings_messages_submenu.close_now();
                self.overlay_menus.settings_input_submenu.close_now();
                self.overlay_menus
                    .settings_language_submenu
                    .set_parent_item_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsLanguageParentExited => {
                self.overlay_menus
                    .settings_language_submenu
                    .set_parent_item_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::LanguageSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsLanguageSubmenuEntered => {
                self.overlay_menus
                    .settings_language_submenu
                    .set_submenu_zone_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsLanguageSubmenuExited => {
                self.overlay_menus
                    .settings_language_submenu
                    .set_submenu_zone_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::LanguageSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsLanguageCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::LanguageSubmenu,
                    now,
                ) {
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self
                        .settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                    {
                        return command;
                    }
                }
            }
            Message::SettingsFontParentEntered => {
                self.overlay_menus.settings_theme_submenu.close_now();
                self.overlay_menus.settings_language_submenu.close_now();
                self.overlay_menus.settings_ai_models_submenu.close_now();
                self.overlay_menus.settings_messages_submenu.close_now();
                self.overlay_menus.settings_input_submenu.close_now();
                self.overlay_menus
                    .settings_font_submenu
                    .set_parent_item_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsFontParentExited => {
                self.overlay_menus
                    .settings_font_submenu
                    .set_parent_item_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::FontSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsFontSubmenuEntered => {
                self.overlay_menus
                    .settings_font_submenu
                    .set_submenu_zone_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsFontSubmenuExited => {
                self.overlay_menus
                    .settings_font_submenu
                    .set_submenu_zone_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::FontSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsFontCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::FontSubmenu,
                    now,
                ) {
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self
                        .settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                    {
                        return command;
                    }
                }
            }
            Message::SettingsAiModelsParentEntered => {
                self.overlay_menus.settings_theme_submenu.close_now();
                self.overlay_menus.settings_language_submenu.close_now();
                self.overlay_menus.settings_font_submenu.close_now();
                self.overlay_menus.settings_messages_submenu.close_now();
                self.overlay_menus.settings_input_submenu.close_now();
                self.overlay_menus
                    .settings_ai_models_submenu
                    .set_parent_item_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsAiModelsParentExited => {
                self.overlay_menus
                    .settings_ai_models_submenu
                    .set_parent_item_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::AiModelsSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsAiModelsSubmenuEntered => {
                self.overlay_menus
                    .settings_ai_models_submenu
                    .set_submenu_zone_active(true, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsAiModelsSubmenuExited => {
                self.overlay_menus
                    .settings_ai_models_submenu
                    .set_submenu_zone_active(false, Instant::now());
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::AiModelsSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsAiModelsCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::AiModelsSubmenu,
                    now,
                ) {
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self
                        .settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                    {
                        return command;
                    }
                }
            }
            Message::SettingsMessagesParentEntered => {
                self.overlay_menus.settings_theme_submenu.close_now();
                self.overlay_menus.settings_language_submenu.close_now();
                self.overlay_menus.settings_font_submenu.close_now();
                self.overlay_menus.settings_ai_models_submenu.close_now();
                let input_branch_active = self
                    .overlay_menus
                    .settings_input_submenu
                    .keeps_parent_branch_open();
                self.overlay_menus
                    .settings_messages_submenu
                    .set_parent_item_active(true, Instant::now(), input_branch_active);
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsMessagesParentExited => {
                let input_branch_active = self
                    .overlay_menus
                    .settings_input_submenu
                    .keeps_parent_branch_open();
                self.overlay_menus
                    .settings_messages_submenu
                    .set_parent_item_active(false, Instant::now(), input_branch_active);
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::MessagesSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsMessagesSubmenuEntered => {
                let input_branch_active = self
                    .overlay_menus
                    .settings_input_submenu
                    .keeps_parent_branch_open();
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(true, Instant::now(), input_branch_active);
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsMessagesSubmenuExited => {
                let input_branch_active = self
                    .overlay_menus
                    .settings_input_submenu
                    .keeps_parent_branch_open();
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(false, Instant::now(), input_branch_active);
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::MessagesSubmenu,
                    SettingsOverlayMenuPanel::InputSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsMessagesCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::MessagesSubmenu,
                    now,
                ) {
                    self.overlay_menus.settings_input_submenu.close_now();
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self
                        .settings_overlay_pending_close_command(&[SettingsOverlayMenuPanel::First])
                    {
                        return command;
                    }
                }
            }
            Message::SettingsInputParentEntered => {
                self.overlay_menus
                    .settings_input_submenu
                    .set_parent_item_active(true, Instant::now());
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(true, Instant::now(), true);
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsInputParentExited => {
                self.overlay_menus
                    .settings_input_submenu
                    .set_parent_item_active(false, Instant::now());
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(false, Instant::now(), true);
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::MessagesSubmenu,
                    SettingsOverlayMenuPanel::InputSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsInputSubmenuEntered => {
                self.overlay_menus
                    .settings_input_submenu
                    .set_submenu_zone_active(true, Instant::now());
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(true, Instant::now(), true);
                self.overlay_menus
                    .set_settings_menu_zone_active(true, Instant::now());
            }
            Message::SettingsInputSubmenuExited => {
                self.overlay_menus
                    .settings_input_submenu
                    .set_submenu_zone_active(false, Instant::now());
                self.overlay_menus
                    .settings_messages_submenu
                    .set_submenu_zone_active(false, Instant::now(), true);
                self.overlay_menus
                    .set_settings_menu_zone_active(false, Instant::now());
                if let Some(command) = self.settings_overlay_pending_close_command(&[
                    SettingsOverlayMenuPanel::First,
                    SettingsOverlayMenuPanel::MessagesSubmenu,
                    SettingsOverlayMenuPanel::InputSubmenu,
                ]) {
                    return command;
                }
            }
            Message::SettingsInputCloseDelayTick(now) => {
                if self.settings_overlay_menu_close_if_pending_expired(
                    SettingsOverlayMenuPanel::InputSubmenu,
                    now,
                ) {
                    self.overlay_menus.refresh_settings_menu_branch(now);
                    self.state.update_technical_output(self.logger.get_logs());
                    if let Some(command) = self.settings_overlay_pending_close_command(&[
                        SettingsOverlayMenuPanel::First,
                        SettingsOverlayMenuPanel::MessagesSubmenu,
                    ]) {
                        return command;
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        // Рендер читає локалізацію й тему, але не змінює доменний стан.
        let ui_theme = Self::load_active_theme_colors();

        let settings_title_text = self.localized_string_or("ui.label_settings", "Settings");
        let settings_button_text =
            self.localized_string_or("ui.button_settings", &settings_title_text);
        let debug_button_text = self.localized_string_or("ui.button_debug", "Debug");
        let logs_title_text = self.localized_string_or("ui.label_logs", "Logs");
        let dialogue_placeholder = self.localized_string_or("ui.placeholder_input", "Enter query");
        let send_button_text = self.localized_string_or("ui.button_send", "Send");

        self.render_scene_layers(
            &ui_theme.colors,
            settings_title_text,
            settings_button_text,
            debug_button_text,
            logs_title_text,
            dialogue_placeholder,
            send_button_text,
        )
    }
}

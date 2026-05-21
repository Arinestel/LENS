// Цей файл є точкою входу в LENS Desktop Shell v0.
// Він підключає модулі застосунку і запускає головне UI-вікно.

mod interface;
mod core;
mod logging;
mod orchestrator;
mod state;
mod ui;

use iced::Application;
use ui::App;

fn main() -> iced::Result {
    // Передаємо керування фреймворку iced, який створює вікно й цикл подій.
    App::run(iced::Settings::default())
}

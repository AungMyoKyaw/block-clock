#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod clock;
mod platform;
mod settings;
mod theme;
mod ui;

use gpui::{
    App, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, actions, point, prelude::*,
    px, size,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};

use crate::settings::{Settings, default_settings_path};
use crate::ui::{
    BlockClockView, Escape, JumpToNow, MoveDown, MoveLeft, MoveRight, MoveRowEnd, MoveRowStart,
    MoveUp, OpenSettings, ToggleFullscreen, TogglePin,
};

actions!(block_clock, [Quit]);
#[cfg(target_os = "macos")]
actions!(block_clock, [About]);

#[cfg(target_os = "macos")]
fn set_up_app_menu(cx: &mut App) {
    cx.on_action(|_: &About, _| platform::show_about_panel());
    cx.set_menus(vec![Menu {
        name: "Block Clock".into(),
        items: vec![
            MenuItem::action("About Block Clock", About),
            MenuItem::separator(),
            MenuItem::action("Quit Block Clock", Quit),
        ],
    }]);
}

fn main() {
    let (initial_settings, initial_notice) = Settings::load_default();
    let settings_path = default_settings_path();

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            platform::set_app_icon(include_bytes!("../assets/AppIcon.icns"));
            cx.bind_keys([
                KeyBinding::new("cmd-p", TogglePin, None),
                KeyBinding::new("ctrl-p", TogglePin, None),
                KeyBinding::new("cmd-shift-f", ToggleFullscreen, None),
                KeyBinding::new("ctrl-shift-f", ToggleFullscreen, None),
                KeyBinding::new("cmd-,", OpenSettings, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
                KeyBinding::new("escape", Escape, None),
                KeyBinding::new("j", JumpToNow, None),
                KeyBinding::new("left", MoveLeft, None),
                KeyBinding::new("right", MoveRight, None),
                KeyBinding::new("up", MoveUp, None),
                KeyBinding::new("down", MoveDown, None),
                KeyBinding::new("home", MoveRowStart, None),
                KeyBinding::new("end", MoveRowEnd, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);

            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            #[cfg(target_os = "macos")]
            set_up_app_menu(cx);

            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = match (
                initial_settings.window_x,
                initial_settings.window_y,
                initial_settings.window_w,
                initial_settings.window_h,
            ) {
                (Some(x), Some(y), Some(w), Some(h)) if w >= 720.0 && h >= 520.0 => {
                    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
                }
                _ => Bounds::centered(None, size(px(1180.), px(720.)), cx),
            };

            let pin = initial_settings.pin;
            let settings_for_view = initial_settings.clone();
            let path_for_view = settings_path.clone();
            let notice_for_view = initial_notice.clone();

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.), px(520.))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Block Clock".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    platform::set_always_on_top(window, pin);
                    cx.new(|cx| {
                        BlockClockView::new(
                            window,
                            cx,
                            settings_for_view,
                            path_for_view,
                            notice_for_view,
                        )
                    })
                },
            )
            .expect("failed to open Block Clock window");

            cx.activate(true);
        });
}

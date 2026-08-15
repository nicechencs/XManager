//! XManager desktop entry (GPUI 0.2.2).

mod app;
mod theme;
mod views;
mod widgets;

use app::AppState;
use gpui::{
    prelude::*, px, size, App, Application, Bounds, Context, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};
use xmanager_core::logging::{self, events, Outcome, Stream};

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep the shell layout mode aligned with the live viewport each frame.
        let width: f32 = window.viewport_size().width.into();
        let mode = app::LayoutMode::from_width(width);
        if self.layout_mode != mode {
            self.layout_mode = mode;
        }
        views::render_root(self, cx)
    }
}

fn main() {
    let logging_ok = logging::init_best_effort();
    logging::info(Stream::App, events::APP_START)
        .outcome(if logging_ok {
            Outcome::Ok
        } else {
            Outcome::Error
        })
        .field("version", env!("CARGO_PKG_VERSION"))
        .field("file_logging", logging_ok)
        .emit();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("XManager".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| AppState::new(cx)),
        )
        .expect("failed to open XManager window");
        cx.activate(true);
    });
}

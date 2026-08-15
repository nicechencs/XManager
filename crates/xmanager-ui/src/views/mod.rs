//! View modules composing the main window layout.

mod insights;
mod root;
mod stats_panel;
mod status_bar;
mod toolbar;
mod tweet_list;

pub use insights::render_insights;
pub use root::render_root;

//! My Linux graphical application.

pub mod app;
pub mod state;

pub use app::MyLinuxApp;
pub use state::{GuiState, default_root};

pub const APP_NAME: &str = "My Linux";

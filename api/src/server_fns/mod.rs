use dioxus::prelude::*;

pub mod auth;
pub mod discovery;
pub mod download;
pub mod folder;
pub mod guard;
pub mod navidrome;
pub mod search;
pub mod settings;
pub mod system;
pub mod user;

pub use auth::*;
pub use discovery::*;
pub use download::*;
pub use folder::*;
pub use guard::*;
pub use navidrome::*;
pub use search::*;
pub use settings::*;
pub use system::*;
pub use user::*;

pub fn server_error<E: std::fmt::Display>(e: E) -> ServerFnError {
    ServerFnError::ServerError {
        message: e.to_string(),
        code: 500,
        details: None,
    }
}

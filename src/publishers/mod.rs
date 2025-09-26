pub mod console;
pub mod file;
pub mod mastodon;
pub mod telegram;
pub mod utils;

pub use console::ConsolePublisher;
pub use file::FilePublisher;
pub use mastodon::MastodonPublisher;
pub use telegram::RealTelegramApi;
pub use crate::traits::publisher::Publisher;

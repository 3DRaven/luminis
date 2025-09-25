use strum::{EnumString, IntoStaticStr, AsRefStr};
use serde::{Serialize, Deserialize};

/// Перечисление каналов публикации
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, Serialize, Deserialize)]
#[strum(serialize_all = "lowercase")]
pub enum PublisherChannel {
    /// Telegram канал
    Telegram,
    /// Mastodon канал
    Mastodon,
    /// Консольный вывод
    Console,
    /// Файловый вывод
    File,
}

/// Перечисление каналов краулинга
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, IntoStaticStr, AsRefStr, Serialize, Deserialize)]
#[strum(serialize_all = "lowercase")]
pub enum CrawlerChannel {
    /// RSS канал
    Rss,
    /// NPAList канал
    Npalist,
}

impl PublisherChannel {
    /// Получает строковое представление канала
    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    /// Создает PublisherChannel из строки
    pub fn from_str(s: &str) -> Result<Self, strum::ParseError> {
        s.parse()
    }

    /// Получает все доступные каналы публикации
    pub fn all() -> Vec<PublisherChannel> {
        vec![
            PublisherChannel::Telegram,
            PublisherChannel::Mastodon,
            PublisherChannel::Console,
            PublisherChannel::File,
        ]
    }
}

impl std::fmt::Display for PublisherChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl CrawlerChannel {
    /// Получает строковое представление канала
    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    /// Создает CrawlerChannel из строки
    pub fn from_str(s: &str) -> Result<Self, strum::ParseError> {
        s.parse()
    }

    /// Получает все доступные каналы краулинга
    pub fn all() -> Vec<CrawlerChannel> {
        vec![
            CrawlerChannel::Rss,
            CrawlerChannel::Npalist,
        ]
    }
}

impl std::fmt::Display for CrawlerChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publisher_channel_string_conversion() {
        assert_eq!(PublisherChannel::Telegram.as_str(), "telegram");
        assert_eq!(PublisherChannel::Mastodon.as_str(), "mastodon");
        assert_eq!(PublisherChannel::Console.as_str(), "console");
        assert_eq!(PublisherChannel::File.as_str(), "file");
    }

    #[test]
    fn test_publisher_channel_from_string() {
        assert_eq!(PublisherChannel::from_str("telegram").unwrap(), PublisherChannel::Telegram);
        assert_eq!(PublisherChannel::from_str("mastodon").unwrap(), PublisherChannel::Mastodon);
        assert_eq!(PublisherChannel::from_str("console").unwrap(), PublisherChannel::Console);
        assert_eq!(PublisherChannel::from_str("file").unwrap(), PublisherChannel::File);
    }

    #[test]
    fn test_publisher_channel_display() {
        assert_eq!(format!("{}", PublisherChannel::Telegram), "telegram");
        assert_eq!(format!("{}", PublisherChannel::Mastodon), "mastodon");
    }

    #[test]
    fn test_publisher_channel_all() {
        let all_channels = PublisherChannel::all();
        assert_eq!(all_channels.len(), 4);
        assert!(all_channels.contains(&PublisherChannel::Telegram));
        assert!(all_channels.contains(&PublisherChannel::Mastodon));
        assert!(all_channels.contains(&PublisherChannel::Console));
        assert!(all_channels.contains(&PublisherChannel::File));
    }

    #[test]
    fn test_crawler_channel_string_conversion() {
        assert_eq!(CrawlerChannel::Rss.as_str(), "rss");
        assert_eq!(CrawlerChannel::Npalist.as_str(), "npalist");
    }

    #[test]
    fn test_crawler_channel_from_string() {
        assert_eq!(CrawlerChannel::from_str("rss").unwrap(), CrawlerChannel::Rss);
        assert_eq!(CrawlerChannel::from_str("npalist").unwrap(), CrawlerChannel::Npalist);
    }

    #[test]
    fn test_crawler_channel_display() {
        assert_eq!(format!("{}", CrawlerChannel::Rss), "rss");
        assert_eq!(format!("{}", CrawlerChannel::Npalist), "npalist");
    }

    #[test]
    fn test_crawler_channel_all() {
        let all_channels = CrawlerChannel::all();
        assert_eq!(all_channels.len(), 2);
        assert!(all_channels.contains(&CrawlerChannel::Rss));
        assert!(all_channels.contains(&CrawlerChannel::Npalist));
    }
}

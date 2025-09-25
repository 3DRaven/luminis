use crate::services::settings::AppConfig;
use crate::models::channel::PublisherChannel;
use std::collections::HashMap;
use bon::bon;

/// Определение канала публикации с его лимитами
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub channel: PublisherChannel,
    pub max_chars: usize,
    pub enabled: bool,
}

/// Менеджер каналов публикации
pub struct ChannelManager {
    channels: HashMap<PublisherChannel, ChannelConfig>,
}

#[bon]
impl ChannelManager {
    #[builder]
    pub fn new(config: &AppConfig) -> Self {
        let mut channels = HashMap::new();

        // Telegram канал
        if let Some(telegram) = &config.telegram {
            channels.insert(PublisherChannel::Telegram, ChannelConfig {
                channel: PublisherChannel::Telegram,
                max_chars: telegram.max_chars.unwrap_or(4096),
                enabled: telegram.enabled,
            });
        }

        // Mastodon канал
        if let Some(mastodon) = &config.mastodon {
            channels.insert(PublisherChannel::Mastodon, ChannelConfig {
                channel: PublisherChannel::Mastodon,
                max_chars: mastodon.max_chars.unwrap_or(495),
                enabled: mastodon.enabled,
            });
        }

        // Console канал
        if let Some(output) = &config.output {
            channels.insert(PublisherChannel::Console, ChannelConfig {
                channel: PublisherChannel::Console,
                max_chars: output.console_max_chars.unwrap_or(10000),
                enabled: output.console_enabled.unwrap_or(true),
            });
        }

        // File канал
        if let Some(output) = &config.output {
            channels.insert(PublisherChannel::File, ChannelConfig {
                channel: PublisherChannel::File,
                max_chars: output.file_max_chars.unwrap_or(20000),
                enabled: output.file_enabled.unwrap_or(false),
            });
        }

        Self { channels }
    }

    /// Получает список всех включенных каналов
    pub fn get_enabled_channels(&self) -> Vec<&ChannelConfig> {
        self.channels.values().filter(|c| c.enabled).collect()
    }

    /// Получает конфигурацию канала по имени
    pub fn get_channel(&self, channel: PublisherChannel) -> Option<&ChannelConfig> {
        self.channels.get(&channel)
    }

    /// Получает список всех каналов (включенных и отключенных)
    pub fn get_all_channels(&self) -> Vec<&ChannelConfig> {
        self.channels.values().collect()
    }

    /// Проверяет, включен ли канал
    pub fn is_channel_enabled(&self, channel: PublisherChannel) -> bool {
        self.channels.get(&channel).map(|c| c.enabled).unwrap_or(false)
    }

    /// Получает лимит символов для канала
    pub fn get_channel_limit(&self, channel: PublisherChannel) -> Option<usize> {
        self.channels.get(&channel).map(|c| c.max_chars)
    }
}

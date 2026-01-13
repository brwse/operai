//! Type definitions for Discord API structures.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Discord channel type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
pub enum ChannelType {
    /// Text channel within a server.
    GuildText = 0,
    /// Direct message between users.
    Dm = 1,
    /// Voice channel within a server.
    GuildVoice = 2,
    /// Direct message between multiple users.
    GroupDm = 3,
    /// Organizational category that contains channels.
    GuildCategory = 4,
    /// Channel for announcements.
    GuildAnnouncement = 5,
    /// Thread within a `GuildAnnouncement` channel.
    AnnouncementThread = 10,
    /// Temporary sub-channel within a `GuildText` channel.
    PublicThread = 11,
    /// Temporary sub-channel within a `GuildText` channel (private).
    PrivateThread = 12,
    /// Voice channel for stage events.
    GuildStageVoice = 13,
    /// Channel in hub for server directories.
    GuildDirectory = 14,
    /// Channel for hosting forum posts.
    GuildForum = 15,
    /// Channel for media posts.
    GuildMedia = 16,
}

impl From<ChannelType> for u8 {
    fn from(ct: ChannelType) -> u8 {
        ct as u8
    }
}

impl TryFrom<u8> for ChannelType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChannelType::GuildText),
            1 => Ok(ChannelType::Dm),
            2 => Ok(ChannelType::GuildVoice),
            3 => Ok(ChannelType::GroupDm),
            4 => Ok(ChannelType::GuildCategory),
            5 => Ok(ChannelType::GuildAnnouncement),
            10 => Ok(ChannelType::AnnouncementThread),
            11 => Ok(ChannelType::PublicThread),
            12 => Ok(ChannelType::PrivateThread),
            13 => Ok(ChannelType::GuildStageVoice),
            14 => Ok(ChannelType::GuildDirectory),
            15 => Ok(ChannelType::GuildForum),
            16 => Ok(ChannelType::GuildMedia),
            _ => Err(format!("Unknown channel type: {value}")),
        }
    }
}

/// Represents a Discord channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Channel {
    /// Channel ID.
    pub id: String,
    /// Type of channel.
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    /// Channel name.
    #[serde(default)]
    pub name: Option<String>,
    /// Guild ID (if applicable).
    #[serde(default)]
    pub guild_id: Option<String>,
    /// Sorting position.
    #[serde(default)]
    pub position: Option<i32>,
    /// Channel topic.
    #[serde(default)]
    pub topic: Option<String>,
    /// Whether the channel is NSFW.
    #[serde(default)]
    pub nsfw: Option<bool>,
    /// ID of the parent category.
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// Represents a Discord user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    /// User ID.
    pub id: String,
    /// Username (not unique across platform).
    pub username: String,
    /// User's display name.
    #[serde(default)]
    pub global_name: Option<String>,
    /// User's avatar hash.
    #[serde(default)]
    pub avatar: Option<String>,
    /// Whether the user is a bot.
    #[serde(default)]
    pub bot: Option<bool>,
}

/// Represents a message attachment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Attachment {
    /// Attachment ID.
    pub id: String,
    /// Name of the file.
    pub filename: String,
    /// Size of file in bytes.
    pub size: u64,
    /// Source URL of file.
    pub url: String,
    /// Proxied URL of file.
    pub proxy_url: String,
    /// Content type.
    #[serde(default)]
    pub content_type: Option<String>,
}

/// Represents a Discord message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    /// Message ID.
    pub id: String,
    /// Channel ID the message was sent in.
    pub channel_id: String,
    /// Author of the message.
    pub author: User,
    /// Contents of the message.
    pub content: String,
    /// When the message was sent (ISO8601 timestamp).
    pub timestamp: String,
    /// When the message was edited (ISO8601 timestamp).
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    /// Whether this was a TTS message.
    #[serde(default)]
    pub tts: bool,
    /// Whether this message mentions everyone.
    #[serde(default)]
    pub mention_everyone: bool,
    /// Attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Whether this message is pinned.
    #[serde(default)]
    pub pinned: bool,
}

/// Represents a thread channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Thread {
    /// Thread ID.
    pub id: String,
    /// Thread name.
    pub name: String,
    /// Type of channel.
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    /// Guild ID.
    #[serde(default)]
    pub guild_id: Option<String>,
    /// ID of the parent channel.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Whether the thread is archived.
    #[serde(default)]
    pub archived: Option<bool>,
    /// Whether the thread is locked.
    #[serde(default)]
    pub locked: Option<bool>,
}

/// Thread metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ThreadMetadata {
    /// Whether the thread is archived.
    pub archived: bool,
    /// Duration in minutes to automatically archive after inactivity.
    pub auto_archive_duration: i32,
    /// Timestamp when the thread was archived.
    #[serde(default)]
    pub archive_timestamp: Option<String>,
    /// Whether the thread is locked.
    pub locked: bool,
}

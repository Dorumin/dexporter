use serde::{Deserialize, Serialize, de::Visitor};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U64ReprStr(pub u64);

impl std::ops::Deref for U64ReprStr {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct U64ReprStrVisitor;

impl<'de> Visitor<'de> for U64ReprStrVisitor {
    type Value = U64ReprStr;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A string that encoded a 64-bit unsigned integer")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let parsed = v.parse::<u64>();

        match parsed {
            Ok(u) => Ok(U64ReprStr(u)),
            Err(_) => Err(E::custom(format!("{} is not a valid u64", v)))
        }
    }
}

impl Serialize for U64ReprStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for U64ReprStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        deserializer.deserialize_str(U64ReprStrVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub guild_folders: Vec<GuildFolder>,
    // pub guild_positions: Vec<U64ReprStr>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildFolder {
    pub guild_ids: Vec<U64ReprStr>
}

impl Settings {
    pub fn guild_ids(&self) -> Vec<u64> {
        self.guild_folders.iter().flat_map(|f| f.guild_ids.iter().map(|id| **id)).collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Guild {
    pub id: U64ReprStr,
    pub name: String,
    pub icon: Option<String>,
    pub owner: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: U64ReprStr,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DMChannel {
    pub r#type: i32,
    pub id: U64ReprStr,
    pub last_message_id: Option<U64ReprStr>,
    pub recipients: Vec<User>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChannel {
    pub r#type: i32,
    pub id: U64ReprStr,
    pub guild_id: U64ReprStr,
    pub name: String,
    pub parent_id: Option<U64ReprStr>,
    pub last_message_id: Option<U64ReprStr>,
    pub topic: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Channel {
    DMChannel(DMChannel),
    TextChannel(TextChannel)
}

impl Channel {
    pub fn id(&self) -> u64 {
        match self {
            Channel::DMChannel(dmchannel) => *dmchannel.id,
            Channel::TextChannel(text_channel) => *text_channel.id,
        }
    }

    pub fn is_text(&self) -> bool {
        match self {
            Channel::DMChannel(_) => true,
            Channel::TextChannel(c) => c.r#type == 0
        }
    }

    pub fn last_message_id(&self) -> Option<u64> {
        match self {
            Channel::DMChannel(dmchannel) => dmchannel.last_message_id.map(|id| *id),
            Channel::TextChannel(text_channel) => text_channel.last_message_id.map(|id| *id),
        }
    }

    pub fn display(&self) -> String {
        match self {
            Channel::DMChannel(dmchannel) => {
                let names: Vec<_> = dmchannel.recipients.iter().map(|u| u.username.clone()).collect();

                format!("#DM({})", names.join(", "))
            },
            Channel::TextChannel(text_channel) => format!("#{}", text_channel.name),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: U64ReprStr,
    pub r#type: u32,
    pub timestamp: Option<DateTime<Utc>>,
    pub content: Option<String>,
    pub author: Author,
    pub attachments: Vec<Attachment>,
    pub edited_timestamp: Option<String>,
    pub embeds: Vec<Embed>,
    pub pinned: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Author {
    pub username: String,
    pub avatar: Option<String>,
    pub id: String,
    pub global_name: Option<String>
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub height: Option<usize>,
    pub width: Option<usize>,
    pub content_type: Option<String>,
    pub original_content_type: Option<String>,
    pub size: Option<u64>,
    pub proxy_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Embed {
    r#type: Option<String>,
    author: Option<EmbedAuthor>,
    thumbnail: Option<EmbedThumbnail>,
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    fields: Option<Vec<EmbedField>>,
    footer: Option<EmbedFooter>,
    timestamp: Option<String>,
    color: Option<u32>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedField {
    name: String,
    value: String,
    #[serde(default)]
    inline: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedAuthor {
    name: Option<String>,
    url: Option<String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedThumbnail {
    url: String,
    content_type: Option<String>,
    proxy_url: Option<String>,
    width: Option<usize>,
    height: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbedFooter {
    text: Option<String>,
    icon_url: Option<String>
}

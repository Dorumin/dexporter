use std::fs::OpenOptions;
use std::path::Path;
use std::io::{BufRead, BufReader};

use futures::stream::{self, StreamExt};
use tokio::fs;
use tokio::io::{AsyncWriteExt as _, BufWriter};

use crate::api::fetch_messages;
use crate::args::Update;
use crate::types::{Channel, Message};

// #[derive(Serialize, Deserialize)]
// struct ChannelInfo {
//     id: String,
//     r#type: u32,
//     flags: u32,
//     last_message_id: String,
//     last_pin_timestamp: String,
//     recipients: Option<Vec<Recipient>>
// }



// async fn get_channel_info(channel_id: &str) -> Result<Channel, anyhow::Error> {

// }

#[derive(Debug)]
pub struct ParsedDex {
    pub header: Channel,
    pub messages: imbl::Vector<Message>
}

impl ParsedDex {
    pub async fn parse(file_path: &Path) -> Option<Self> {
        let file = OpenOptions::new()
            .read(true)
            // We need write just to create it
            .write(true)
            .create(true)
            .truncate(false)
            .open(file_path)
            .unwrap();

        let file = BufReader::new(file);

        let mut lines = file.lines();

        let Some(Ok(first_line)) = lines.next() else {
            return None;
        };

        let Ok(header) = serde_json::from_str(&first_line) else {
            eprintln!("Failed to parse channel");

            return None;
        };

        let mut messages = imbl::vector![];

        for line in lines {
            if let Ok(line) = line {
                if let Ok(message) = serde_json::from_str(&line) {
                    messages.push_back(message);
                } else {
                    eprintln!("line failed to parse: {line}");
                }
            } else {
                eprintln!("Failed to read next line. File is likely corrupted.");
            }
        }

        Some(ParsedDex {
            header,
            messages
        })
    }

    pub async fn save(&self, file_path: &Path)  -> anyhow::Result<()> {
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .await?;
        let mut file = BufWriter::new(file);

        file.write_all(serde_json::to_string(&self.header)?.as_bytes()).await?;
        file.write_all(b"\n").await?;

        for message in self.messages.iter() {
            file.write_all(serde_json::to_string(&message).unwrap().as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        Ok(())
    }
}

async fn update_channel(options: &Update, channel: &Channel, file_path: &Path) -> anyhow::Result<()> {
    let mut parsed = ParsedDex::parse(file_path).await.unwrap_or_else(|| ParsedDex {
        header: channel.clone(),
        messages: imbl::vector![]
    });

    // Messages are ordered oldest to newest - start from last, newest message's id
    let mut start_from = parsed.messages.last().map_or(0, |m| *m.id);

    if channel.last_message_id() == Some(start_from) {
        eprintln!("skipping: {}; manifest states last message id is the same as stored", channel.display());
        return Ok(());
    }

    for i in 1u32.. {
        eprintln!("fetching: {} length: {} from: {}", channel.display(), parsed.messages.len(), start_from);

        let messages = fetch_messages(&options.token, channel.id(), start_from).await?;

        if messages.is_empty() {
            break;
        }

        // I don't wanna encode whether the first or last is the latest one
        if let Some(first) = messages.first() {
            start_from = start_from.max(*first.id);
        }

        if let Some(last) = messages.last() {
            start_from = start_from.max(*last.id);
        }

        for message in messages {
            let result = parsed.messages.binary_search_by_key(&message.timestamp, |m| m.timestamp);

            match result {
                Ok(index) => parsed.messages[index] = message,
                Err(index) => parsed.messages.insert(index, message),
            }
        }

        if i.is_multiple_of(100) {
            eprintln!("storing: {}", channel.display());
            parsed.save(file_path).await?;
        }
    }

    parsed.save(file_path).await?;

    Ok(())
}

pub async fn update_channels(options: &Update) {
    let stream = stream::iter(&options.state.channels);

    let concurrent = stream.for_each_concurrent(5, |channel| async move {
        let mut path = std::env::current_dir().unwrap();

        path.push("db");

        let id = match channel {
            Channel::DMChannel(c) => {
                path.push("DMs");

                c.id.to_string()
            },
            Channel::TextChannel(c) => {
                path.push(c.guild_id.to_string());

                c.id.to_string()
            },
        };

        fs::create_dir_all(&path).await.unwrap();

        path.push(format!("{id}.dex"));

        match update_channel(options, channel, &path).await {
            Ok(_) => {},
            Err(e) => {
                eprintln!("update channel: {} failed: {}", channel.display(), e);
            },
        };
    });

    concurrent.await;
}

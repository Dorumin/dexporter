use std::time::Duration;

use reqwest::{Url};
use tokio::join;

use crate::args::Update;
use crate::types::{Channel, DMChannel, Guild, Message, Settings, TextChannel, U64ReprStr};


pub async fn fetch_dms(options: &Update) -> Vec<DMChannel> {
    let client = reqwest::Client::new();
    let mut url = Url::parse("https://discord.com").unwrap();

    url.set_path("api/v6/users/@me/channels");

    client.get(url)
        .header("Authorization", &options.token)
        .send()
        .await.unwrap()
        .json::<Vec<DMChannel>>()
        .await.unwrap()
}

pub async fn fetch_guilds(options: &Update) -> Vec<Guild> {
    let client = reqwest::Client::new();
    let mut guilds_url = Url::parse("https://discord.com").unwrap();
    let mut settings_url = Url::parse("https://discord.com").unwrap();

    guilds_url.set_path("api/v9/users/@me/guilds");
    settings_url.set_path("api/v9/users/@me/settings");

    let (guilds, settings) = join!(
        client.get(guilds_url)
            .header("Authorization", &options.token)
            .send(),
        client.get(settings_url)
            .header("Authorization", &options.token)
            .send()
    );

    let (guilds, settings) = join!(
        guilds.unwrap().json::<Vec<Guild>>(),
        settings.unwrap().json::<Settings>()
    );

    let (mut guilds, settings) = (guilds.unwrap(), settings.unwrap());

    guilds.sort_by_cached_key(|guild|
        // This could be more efficient...
        settings.guild_ids()
            .iter()
            .enumerate()
            .find(|(_, s)| **s == *guild.id)
            .unwrap_or((0, &U64ReprStr(0)))
            .0
    );

    guilds
}

pub async fn fetch_channels(options: &Update, guild_id: u64) -> Vec<TextChannel> {
    let client = reqwest::Client::new();
    let mut url = Url::parse("https://discord.com").unwrap();

    url.set_path(&format!("api/v6/guilds/{guild_id}/channels"));

    client.get(url)
        .header("Authorization", &options.token)
        .send()
        .await.unwrap()
        .json::<Vec<TextChannel>>()
        .await.unwrap()
}

pub async fn fetch_channel(options: &Update, channel_id: u64) -> Channel {
    let client = reqwest::Client::new();
    let mut url = Url::parse("https://discord.com").unwrap();

    url.set_path(&format!("api/v6/channels/{channel_id}"));

    client.get(url)
        .header("Authorization", &options.token)
        .send()
        .await.unwrap()
        .json::<Channel>()
        .await.unwrap()
}


pub async fn fetch_messages(options: &Update, channel_id: u64, start_from: u64) -> anyhow::Result<Vec<Message>> {
    let client = reqwest::Client::new();
    let mut url = Url::parse("https://discord.com").unwrap();

    url.set_path(&format!("api/v6/channels/{channel_id}/messages"));

    url.query_pairs_mut().append_pair("limit", "100");
    url.query_pairs_mut().append_pair("after", &format!["{start_from}"]);

    let mut retries = 0;
    let results = loop {
        let response = client.get(url.clone())
            .header("Authorization", &options.token)
            .send()
            .await;

        let response = match response {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Failed while fetching messages {e:?}");

                if retries > 3 {
                    return Err(e.into());
                }

                retries += 1;

                tokio::time::sleep(Duration::from_secs(retries * 3)).await;

                continue;
            }
        };

        let messages = response.json::<Vec<Message>>().await;

        let messages = match messages {
            Ok(messages) => messages,
            Err(e) => {
                eprintln!("Failed while parsing messages in {channel_id} from {start_from}:\n{e:?}");

                if retries > 3 {
                    return Err(e.into());
                }

                retries += 1;

                tokio::time::sleep(Duration::from_secs(3)).await;

                continue;
            }
        };

        break messages;
    };

    // dbg!(results);

    Ok(results)
}

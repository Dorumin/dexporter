use tokio::io::{self, AsyncBufReadExt, BufReader};
use anyhow::Context;

use crate::api::{fetch_channel, fetch_channels, fetch_dms, fetch_guilds};
use crate::args::Update;
use crate::fs::update_channels;
use crate::types::Channel;

async fn populate_interactive_channels(options: &mut Update) -> Option<()> {
    let mut input_lines = BufReader::new(io::stdin()).lines();

    loop {
        println!("Log DMs? [y/n/quit]");

        let response = input_lines.next_line().await.ok()??;
        match response.trim() {
            "y" | "yes" => {
                let dms = fetch_dms(options).await;

                println!("Added {} DMs", dms.len());

                options.state.channels.reserve(dms.len());
                options.state.channels.extend(
                    dms.into_iter()
                        .map(Channel::DMChannel)
                );
                break;
            },
            "quit" => return None,
            "n" | "no" => break,
            _ => println!("What? I'm going to ask again")
        }
    }

    let guilds = fetch_guilds(options).await;

    for guild in guilds {
        let can_start = !options.state.channels.is_empty();
        let choices = [
            Some("y"),
            Some("n"),
            if can_start {
                Some("start")
            } else {
                None
            },
            Some("quit")
        ];

        loop {
            println!("Log all channels in {name}?{extra} [{choices}]",
                name = guild.name,
                extra = if guild.owner {
                    " (you own this guild)"
                } else {
                    ""
                },
                choices = choices.into_iter().flatten().collect::<Vec<_>>().join("/")
            );

            let response = input_lines.next_line().await.ok()??;

            match response.trim() {
                "y" | "yes" => {
                    let channels = fetch_channels(options, *guild.id).await;
                    let additions: Vec<_> = channels.into_iter()
                        .map(Channel::TextChannel)
                        .filter(|c| c.is_text())
                        .collect();

                    println!("Added {} channels", additions.len());

                    options.state.guilds.push(guild);

                    options.state.channels.reserve(additions.len());
                    options.state.channels.extend(additions);
                    break;
                },
                "quit" => return None,
                "start" if can_start => return Some(()),
                "n" | "no" => break,
                _ => println!("What? I'm going to ask again")
            }
        }
    }

    Some(())
}

pub async fn do_update(mut options: Update) -> Result<(), anyhow::Error> {
    // dbg!(&options.channels);

    if options.channels.is_empty() && options.guilds.is_empty() {
        populate_interactive_channels(&mut options).await.context("what")?;
    } else {
        for channel in options.channels.iter() {
            let channel = fetch_channel(&options, channel.parse().unwrap()).await;

            options.state.channels.push(channel);
        }

        for guild in options.guilds.iter() {
            let channels = fetch_channels(&options, guild.parse().unwrap()).await;

            options.state.channels.reserve(channels.len());
            options.state.channels.extend(
                channels.into_iter()
                    .map(Channel::TextChannel)
            );
        }
    }

    update_channels(&options).await;

    Ok(())
}

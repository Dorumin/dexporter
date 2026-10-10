use std::path::PathBuf;

use crate::api::fetch_messages;
use crate::args::Download;
use crate::fs::ParsedDex;
use crate::types::{Attachment, Message};

pub async fn download_attachment(
    attachment: &Attachment,
    channel: &str,
) -> Result<(), anyhow::Error> {
    tokio::fs::create_dir_all(&format!("download/{channel}")).await?;

    let buffer = reqwest::get(&attachment.url).await?.bytes().await?;

    tokio::fs::write(
        &format!(
            "download/{channel}/{}.{}",
            attachment.id, attachment.filename
        ),
        buffer,
    )
    .await?;

    Ok(())
}

async fn newest_downloaded_id(channel: &str) -> u64 {
    let mut newest = 0;

    // Note: We're deriving message start id from attachment ids. Not too bad, I don't think;
    // attachments probably have older ids than the messages themselves.
    if let Ok(mut files) = tokio::fs::read_dir(&format!("download/{channel}")).await {
        while let Ok(Some(file)) = files.next_entry().await {
            if let Ok(filename) = file.file_name().into_string()
                && let Some((id, _rest)) = filename.split_once('.')
                && let Ok(id) = id.parse()
            {
                newest = newest.max(id);
            }
        }
    }

    newest
}

fn find_export(channel: &str) -> Option<PathBuf> {
    walkdir::WalkDir::new("db").into_iter().find_map(|entry| {
        let entry = entry.ok()?;

        if entry.file_type().is_file()
            && entry.file_name().to_str() == Some(format!("{channel}.dex").as_str())
        {
            Some(entry.into_path())
        } else {
            None
        }
    })
}

fn any_relevant_attachments(message: &Message, regex: Option<&regex::Regex>, floor: u64) -> bool {
    message.attachments.iter().any(|attachment| {
        attachment.id.parse::<u64>().map_or(true, |id| id > floor)
            && regex.is_none_or(|regex| regex.is_match(&attachment.filename))
    })
}

async fn download_channel_ranged(
    channel: &str,
    channel_id: u64,
    token: &str,
    regex: Option<&regex::Regex>,
    parsed: &ParsedDex,
    floor: u64,
) -> Result<(), anyhow::Error> {
    let messages = &parsed.messages;
    let mut index = 0usize;

    // Highest message id the server confirmed to exist. Local ids at or below it were either
    // fetched already or deleted on the server, so there's no point checking them again.
    let mut confirmed = 0u64;

    while let Some(message) = messages.get(index) {
        if *message.id <= confirmed || !any_relevant_attachments(message, regex, floor) {
            index += 1;
            continue;
        }

        // Start fetching right below the relevant message so it lands in the first batch,
        // but never below what the server has already confirmed to exist.
        let fetch_after = index
            .checked_sub(1)
            .map_or(confirmed, |i| (*messages[i].id).max(confirmed));

        eprintln!("fetching: {channel} range from: {fetch_after}");

        let fetched = fetch_messages(token, channel_id, fetch_after).await?;

        if fetched.is_empty() {
            eprintln!("warning: nothing came back for range from {fetch_after}, its attachments are likely gone");

            index += 1;

            continue;
        }

        let last_fetched = fetched.iter().map(|message| *message.id).max().unwrap();

        confirmed = confirmed.max(last_fetched);

        for message in fetched {
            for attachment in message.attachments {
                if let Some(regex) = regex
                    && !regex.is_match(&attachment.filename)
                {
                    continue;
                }

                match download_attachment(&attachment, channel).await {
                    Ok(_) => eprintln!("downloaded: {}", attachment.url),
                    Err(_) => eprintln!("failed: {}", attachment.url),
                }
            }
        }
    }

    Ok(())
}

pub async fn download_channel(channel: &str, args: &Download) -> Result<(), anyhow::Error> {
    let regex = args
        .filter_regex
        .as_ref()
        .map(|filter| regex::Regex::new(filter).unwrap());

    let channel_id: u64 = channel.parse().unwrap();

    let mut start_from = if let Some(start_id) = args.start {
        start_id
    } else {
        let derived = newest_downloaded_id(channel).await;

        if derived > 0 {
            eprintln!(
                "note: starting fetching messages from {derived} due to the newest saved attachments in downloads folder"
            );
            eprintln!("override this with the --start command line argument");
        }

        derived
    };

    if let Some(export_path) = find_export(channel) {
        match ParsedDex::parse(&export_path).await {
            Some(parsed) if !parsed.messages.is_empty() => {
                eprintln!(
                    "note: limiting fetching to message ranges with attachments, according to {}",
                    export_path.display()
                );

                return download_channel_ranged(
                    channel,
                    channel_id,
                    &args.token,
                    regex.as_ref(),
                    &parsed,
                    start_from,
                )
                .await;
            },
            Some(_) => eprintln!(
                "warning: export at {} holds no messages, falling back to a full scan",
                export_path.display()
            ),
            None => eprintln!(
                "warning: failed to parse export at {}, falling back to a full scan",
                export_path.display()
            ),
        }
    }

    for _ in 1u32.. {
        eprintln!("fetching: {} from: {}", channel, start_from);

        let messages = fetch_messages(&args.token, channel_id, start_from).await?;

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
            for attachment in message.attachments {
                if let Some(ref regex) = regex
                    && !regex.is_match(&attachment.filename)
                {
                    continue;
                }

                match download_attachment(&attachment, channel).await {
                    Ok(_) => eprintln!("downloaded: {}", attachment.url),
                    Err(_) => eprintln!("failed: {}", attachment.url),
                }
            }
        }
    }

    Ok(())
}

pub async fn do_download(args: Download) -> Result<(), anyhow::Error> {
    if args.channels.is_empty() {
        return Err(anyhow::anyhow!(
            "The --channels should not be empty. Pass in a list of the ids necessary."
        ));
    }

    for channel in args.channels.iter() {
        download_channel(channel, &args).await?;
    }

    Ok(())
}

use crate::{api::fetch_messages, args::Download, types::Attachment};

pub async fn download_attachment(attachment: &Attachment, channel: &str) -> Result<(), anyhow::Error> {
    tokio::fs::create_dir_all(&format!("download/{channel}")).await?;

    let buffer = reqwest::get(&attachment.url).await?.bytes().await?;

    tokio::fs::write(&format!("download/{channel}/{}.{}", attachment.id, attachment.filename), buffer).await?;

    Ok(())
}

pub async fn download_channel(channel: &str, token: &str) -> Result<(), anyhow::Error> {
    let mut start_from = 0;

    for _ in 1u32.. {
        eprintln!("fetching: {} from: {}", channel, start_from);

        let messages = fetch_messages(token, channel.parse().unwrap(), start_from).await?;

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
        return Err(anyhow::anyhow!("The --channels should not be empty. Pass in a list of the ids necessary."));
    }

    for channel in args.channels {
        download_channel(&channel, &args.token).await?;
    }

    Ok(())
}

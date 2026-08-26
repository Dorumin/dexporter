use crate::{api::fetch_messages, args::Download, types::Attachment};

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

pub async fn download_channel(channel: &str, args: &Download) -> Result<(), anyhow::Error> {
    let regex = args
        .filter_regex
        .as_ref()
        .map(|filter| regex::Regex::new(filter).unwrap());

    let mut start_from = 0;

    if let Some(start_id) = args.start {
        start_from = start_id;
    } else {
        // Note: We're deriving message start id from attachment ids. Not too bad, I don't think;
        // attachments probably have older ids than the messages themselves.
        let files = tokio::fs::read_dir(&format!("download/{channel}")).await;

        if let Ok(mut files) = files {
            while let Ok(Some(file)) = files.next_entry().await {
                if let Ok(filename) = file.file_name().into_string()
                    && let Some((id, _rest)) = filename.split_once('.')
                    && let Ok(id) = id.parse()
                {
                    start_from = start_from.max(id);
                }
            }

            if start_from > 0 {
                eprintln!(
                    "note: starting fetching messages from {start_from} due to the newest saved attachments in downloads folder"
                );
                eprintln!("override this with the --start command line argument");
            }
        }
    }

    for _ in 1u32.. {
        eprintln!("fetching: {} from: {}", channel, start_from);

        let messages = fetch_messages(&args.token, channel.parse().unwrap(), start_from).await?;

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

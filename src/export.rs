use std::{borrow::Cow, path::PathBuf};

use crate::{args::Export, fs::ParsedDex, types::{Channel}};
use chrono::{Datelike, Timelike};
use tokio::io::{AsyncWriteExt, BufWriter};
use walkdir::WalkDir;

pub async fn do_export(export: Export) -> anyhow::Result<()> {
    let mut parsoids = vec![];

    for entry in WalkDir::new("db") {
        let entry = entry?;

        if entry.path().extension().map(|s| s.to_string_lossy()) != Some(Cow::Borrowed("dex")) {
            continue;
        }

        println!("{}", entry.path().display());

        let Some(parsed) = ParsedDex::parse(entry.path()).await else {
            return Err(anyhow::anyhow!("Invalid .dex file."));
        };

        parsoids.push((parsed, entry));
    }

    // Recipients doesn't include self

    // let mut shared_user_ids: Option<HashSet<U64ReprStr>> = None;

    // for (parsed, _) in parsoids.iter() {
    //     if let Channel::DMChannel(dm) = &parsed.header {
    //         let ourset = HashSet::from_iter(dm.recipients.iter().map(|r| r.id));

    //         eprintln!("{ourset:?}");

    //         if let Some(shared) = shared_user_ids {
    //             shared_user_ids = Some(shared.intersection(&ourset).copied().collect());
    //         } else {
    //             shared_user_ids = Some(ourset);
    //         }
    //     }
    // }

    // if let Some(shared) = &shared_user_ids {
    //     assert_eq!(shared.len(), 1);
    // }

    for (parsed, entry) in parsoids {
        let mut components = entry.path().components();
        components.next();
        components.next_back();

        let mut non_db_path = PathBuf::new();
        non_db_path.push("export");
        non_db_path.push(components.as_path());

        let names: String = match &parsed.header {
            Channel::DMChannel(dm) => dm.recipients.iter()
                // .filter(|r| shared_user_ids.as_ref().unwrap().contains(&r.id))
                .map(|r| r.username.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            Channel::TextChannel(text_channel) => text_channel.name.to_string(),
        };

        tokio::fs::create_dir_all(&non_db_path).await.unwrap();

        non_db_path.push(format!("{}.txt", names));

        eprintln!("{} {}", non_db_path.display(), parsed.messages.len());

        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(non_db_path)
            .await?;
        let mut file = BufWriter::new(file);

        let mut last_date = None;

        for message in parsed.messages.iter() {
            let ts = message.timestamp.unwrap();
            let year = ts.year();
            let month = ts.month();
            let day = ts.day();
            let hour = ts.hour();
            let minute = ts.minute();
            let second = ts.second();
            let author = &message.author.username;

            if export.headers && (last_date.is_none() || Some(ts.date_naive()) != last_date) {
                let month_name = match month {
                    1 => "January",
                    2 => "February",
                    3 => "March",
                    4 => "April",
                    5 => "May",
                    6 => "June",
                    7 => "July",
                    8 => "August",
                    9 => "September",
                    10 => "October",
                    11 => "November",
                    12 => "December",
                    _ => unreachable!()
                };

                file.write_all(format!("\n---- {day:02} {month_name} {year:02} ----\n\n").as_bytes()).await?;

                last_date = Some(ts.date_naive());
            }

            file.write_all(format!("{year:02}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} {author}").as_bytes()).await?;

            if let Some(content) = &message.content {
                file.write_all(format!(": {content}").as_bytes()).await?;
            }

            for attachment in &message.attachments {
                file.write_all(format!("\n{}", attachment.url).as_bytes()).await?;
            }

            file.write_all(b"\n").await?;
        }
    }

    Ok(())
}

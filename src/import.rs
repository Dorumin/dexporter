use std::{collections::{HashMap}, time::Duration};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, SubsecRound, Utc};
use tokio::io::{AsyncBufReadExt as _, BufReader};

use crate::{args::Import, fs::ParsedDex, types::{Attachment, Author, Message, U64ReprStr}};

struct TempMessage<'a> {
    timestamp: chrono::DateTime<chrono::Utc>,
    username: &'a str,
    text: String,
    attachments: Vec<&'a str>
}

fn parse_temp_message<'a>(lines: &[&'a str], date: NaiveDate, allowed_users: Option<&[String]>) -> Option<TempMessage<'a>> {
    if lines.is_empty() {
        return None;
    }

    let first = lines[0];
    let first = first.strip_prefix('[')?;


    let removed = first.trim_start_matches(|c: char| c.is_ascii_digit());
    let hours: u32 = first[0..first.len() - removed.len()].parse().ok()?;
    let first = removed.strip_prefix(':')?;

    let removed = first.trim_start_matches(|c: char| c.is_ascii_digit());
    let minutes: u32 = first[0..first.len() - removed.len()].parse().ok()?;
    let first = removed.strip_prefix(':')?;

    let removed = first.trim_start_matches(|c: char| c.is_ascii_digit());
    let seconds: u32 = first[0..first.len() - removed.len()].parse().ok()?;

    let timestamp = NaiveDateTime::new(
        date,
        NaiveTime::from_hms_opt(hours, minutes, seconds)?
    );

    let timestamp = timestamp.and_local_timezone(Utc).single()?;

    let first = removed.strip_prefix("] ")?;

    // This removes up to the first :, which might not exist (like file uploads or non-content messages)
    let removed = first.trim_start_matches(|c| c != ':');
    let username = &first[0..first.len() - removed.len()];

    if allowed_users.is_some_and(|n| !n.iter().any(|u| u == username)) {
        eprintln!("IGNORED: {username}");

        return None;
    }

    let first = removed.strip_prefix(": ").unwrap_or("");
    let mut text = String::from(first);

    let empty_line_count = lines.iter().rev().take_while(|line| line.is_empty()).count();

    let mut attachments: Vec<_> = lines.iter().rev()
        .copied()
        // Very likely to have trailing empty lines. Skip them.
        .skip(empty_line_count)
        .take_while(|line|
            // Very basic heuristic for discord attachments
            line.starts_with("https://cdn.discordapp.com/attachments/")
        )
        .collect();

    if text.starts_with("https://cdn.discordapp.com/attachments/") {
        // If it's multiple "attachments", links, with text on the first line (after a colon) that looks like one,
        // then assume they're all actual links and not files
        attachments.clear();
    }

    for line in lines.iter().skip(1).take(lines.len() - 1 - attachments.len() - empty_line_count) {
        text.push('\n');
        text.push_str(line);
    }

    text = text.trim().to_string();

    let message = TempMessage {
        timestamp,
        username,
        text,
        attachments
    };

    Some(message)
}

fn parse_with_headers<'a>(s: &'a str, allowed_users: Option<&[String]>) -> Option<Vec<TempMessage<'a>>> {
    let mut date: Option<NaiveDate> = None;
    let mut messages = vec![];
    let mut acc = vec![];

    for line in s.lines() {
        if line.starts_with("---- ") && line.ends_with(" ----") {
            let dater = |olddate: Option<NaiveDate>| {
                let inner = &line[5..line.len() - 5];
                let (day, rest) = inner.split_once(' ')?;
                let (month, year) = rest.split_once(' ')?;

                let month = match month {
                    "January" => 1,
                    "February" => 2,
                    "March" => 3,
                    "April" => 4,
                    "May" => 5,
                    "June" => 6,
                    "July" => 7,
                    "August" => 8,
                    "September" => 9,
                    "October" => 10,
                    "November" => 11,
                    "December" => 12,
                    _ => return None
                };

                let newdate = NaiveDate::from_ymd_opt(year.parse().ok()?, month, day.parse().ok()?);

                if newdate < olddate {
                    // panic!("{newdate:?}");
                    // This isn't foolproof, but we do this to filter out quotes in the chat of these kinds of logs
                    // Putting "---- 7 June 2019 ----" verbatim in a text message, on its own line. No time travel.
                    None
                } else {
                    newdate
                }
            };

            if let Some(newdate) = dater(date) {
                if let Some(olddate) = date && let Some(message) = parse_temp_message(&acc, olddate, allowed_users) {
                    messages.push(message);
                }

                acc.clear();

                date = Some(newdate);
                continue;
            }
        }

        // Current line is independently parseable, dump the accumulator into a message
        if date.is_some() && let Some(_) = parse_temp_message(&[line], date.unwrap(), allowed_users) {
            if !acc.is_empty() {
                if acc[0] == "[03:14:05] YouWillAlwaysBeLovedJasonGrace!: My God." {
                    dbg!(&line);
                    dbg!(&acc);
                }

                let parsed = parse_temp_message(&acc, date.unwrap(), allowed_users).expect("we shouldn't fail to parse an accumulator");

                messages.push(parsed);

                acc.clear();
            } else {
                // eprintln!("bub");
            }
        }

        // Not a date header
        acc.push(line);
    }

    if !acc.is_empty() {
        let parsed = parse_temp_message(&acc, date.unwrap(), allowed_users).expect("we shouldn't fail to parse an accumulator at the end");

        messages.push(parsed);
    }

    Some(messages)
}

pub async fn do_import(import: Import) -> anyhow::Result<()> {
    let s = tokio::fs::read_to_string(&import.file).await?;

    let mut messages = parse_with_headers(&s, None).expect("failed to parse");

    eprintln!("parsing success? extracted {} messages", messages.len());

    // messages.sort_by_key(|m| m.text.split('\n').count());
    // messages.sort_by_key(|m| m.attachments.len());

    // let mut test_output = String::new();
    // for message in messages {
    //     test_output.push_str(&message.timestamp.to_string());
    //     test_output.push_str(" - ");

    //     test_output.push_str(message.username);
    //     test_output.push_str(": ");
    //     test_output.push_str(&message.text);

    //     if !message.attachments.is_empty() {
    //         test_output.push_str(&format!("\n{} attachments:\n", message.attachments.len()));

    //         for attachment in message.attachments {
    //             test_output.push_str(attachment);
    //             test_output.push('\n');
    //         }
    //     }

    //     test_output.push('\n');
    // }

    // std::fs::write("bruh.txt", test_output.as_bytes()).unwrap();

    let mut user_counts: HashMap<&str, usize> = HashMap::new();
    for message in &messages {
        *user_counts.entry(message.username).or_default() += 1;
    }

    eprintln!("There were {} unique usernames in this file.", user_counts.len());

    let mut count_vec: Vec<(&str, usize)> = user_counts.into_iter().collect();
    count_vec.sort_by_key(|a| a.1);

    for (user, count) in count_vec {
        eprintln!("{user} has {count} parsed messages.");
    }

    let mut input_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut username_to_id = HashMap::new();

    eprintln!();
    eprintln!("I need you type out a user's name, a colon, and then the user id, with no spaces. The final username and display name will be made up.");
    eprintln!("The file will be re-parsed and ignore any missing usernames (parse them as text). For DMs, this just means two guys.");

    loop {
        let line = input_lines.next_line().await?.unwrap_or_else(String::new);

        if line.is_empty() {
            break;
        }

        let Some((username, id)) = line.split_once(":") else {
            return Err(anyhow::anyhow!("I was pretty clear on the format: username:id"));
        };

        username_to_id.insert(username.to_string(), id.to_string());

        eprintln!("Added user: {username} with id: {id}. Add another, or press enter to rescan and only use these guys.");
    }

    let allowed_users: Vec<_> = username_to_id.keys().cloned().collect();

    messages = parse_with_headers(&s, Some(&allowed_users)).expect("failed to parse");

    eprintln!("If this channel dump is from a guild, paste its id. Otherwise, leave it blank and press enter.");

    let guild_id = input_lines.next_line().await?.unwrap_or_else(String::new);

    if guild_id.is_empty() {
        eprintln!("Chosen kind: DM");
    } else {
        eprintln!("Chosen kind: guild with id {guild_id}");
    }

    eprintln!("It doesn't matter either way. Now paste the id of the channel.");

    let Some(channel_id) = input_lines.next_line().await? else {
        eprintln!("I wasn't kidding.");

        return Err(anyhow::anyhow!("No valid channel id"));
    };

    let mut path = std::env::current_dir().unwrap();

    path.push("db");

    match guild_id.as_str() {
        "" => {
            path.push("DMs");
        }
        guild_id => {
            path.push(guild_id);
        }
    };

    path.push(format!("{channel_id}.dex"));

    let Some(mut parsed) = ParsedDex::parse(&path).await else {
        eprintln!("Couldn't find or parse the .dex file. Importing from scratch is not supported.");

        return Err(anyhow::anyhow!("No .dex file found"));
    };

    eprintln!("Parsed! Hefty.");

    let mut added = 0;

    for temp_message in messages {
        let start = temp_message.timestamp - Duration::from_secs(1);
        let end = temp_message.timestamp + Duration::from_secs(1);

        let start_index = parsed.messages.binary_search_by_key(
            &Some(start),
            |m| m.timestamp
        );
        let start_index = match start_index {
            Ok(index) => index,
            Err(index) => index,
        };
        let end_index = parsed.messages.binary_search_by_key(
            &Some(end),
            |m| m.timestamp
        );
        let end_index = match end_index {
            Ok(index) => index,
            Err(index) => index,
        };

        // .slice() on imbl::Vector takes out the elements (!!)
        let possible_matches: Vec<_> = (start_index..=end_index).map(|index| &parsed.messages[index]).collect();

        // let possible_matches = &parsed.messages.slice(start_index..=end_index);

        let is_doop = possible_matches.iter()
            .any(|m|
                m.content.as_ref().is_some_and(|s| *s == temp_message.text) &&
                // Match timestamps up to the second
                m.timestamp.as_ref().is_some_and(|d|
                    d.trunc_subsecs(0) == temp_message.timestamp.trunc_subsecs(0)
                )
            );

        if temp_message.text == "I can never tell with you always being invis" || temp_message.text == "beck" || temp_message.text == ":c" {
            // dbg!(possible_matches);
            eprintln!("{is_doop} {} {} {}; {start_index}..={end_index}", temp_message.username, temp_message.text, temp_message.timestamp);
        }

        if !is_doop {
            let insert_index = parsed.messages.binary_search_by_key(
                &Some(temp_message.timestamp),
                |m| m.timestamp
            );
            let insert_index = match insert_index {
                Ok(index) => index,
                Err(index) => index,
            };
            let fake_message = Message {
                id: U64ReprStr(0),
                r#type: 0,
                timestamp: Some(temp_message.timestamp),
                attachments: temp_message.attachments.iter().map(|url| Attachment {
                    id: String::from("0"),
                    filename: String::new(),
                    url: url.to_string(),
                    height: None,
                    width: None,
                    content_type: None,
                    original_content_type: None,
                    size: None,
                    proxy_url: None,
                }).collect(),
                author: Author {
                    username: temp_message.username.to_string(),
                    avatar: None,
                    id: username_to_id.get(temp_message.username).unwrap().clone(),
                    global_name: None,
                },
                content: Some(temp_message.text),
                edited_timestamp: None,
                embeds: vec![],
                pinned: None,
            };

            parsed.messages.insert(insert_index, fake_message);

            added += 1;
        }
    }

    eprintln!("Imported {added} new messages! Saving now. It might be slow. Even on a fast ssd.");

    parsed.save(&path).await?;

    Ok(())
}

use futures::stream::StreamExt;
use rand::{thread_rng, Rng};
use std::fs::read_dir;
use telegram_bot::*;

mod background;
mod data;

pub use self::data::*;

#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv();
    let token = std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    background::spawn(token.clone());

    let api = Api::new(token);

    // Fetch new updates via long poll method
    let mut stream = api.stream();
    while let Some(update) = stream.next().await {
        // If the received update contains a new message...
        let update = update.unwrap();
        if let UpdateKind::Message(message) = update.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                let reply_message = reply(data, &message.from).await;
                api.send(SendMessage::new(message.from, reply_message))
                    .await
                    .unwrap();
            }
        }
    }
}

async fn reply(message: &str, sender: &User) -> String {
    let space_index = message
        .chars()
        .position(|c| c == ' ')
        .unwrap_or(message.len());
    let (left, right) = message.split_at(space_index);
    match left.to_ascii_lowercase().as_str() {
        "/start" => "Hello! Add rss feeds by typing /add <url>".to_owned(),
        "/add" => add(sender, right),
        "/status" => status(sender, right),
        "/list" => list(sender).await,
        "/edit" => edit(sender, right).await,
        "/delete" => delete(sender, right),
        _ => "Unknown command.\nyou can list your rss feeds by typing /list.\nYou can add a new rss feed by typing /add <RSS url>".to_owned()
    }
}

fn add(sender: &User, url: &str) -> String {
    let id: i64 = sender.id.into();
    let pin: i64 = thread_rng().gen_range(1111..=9999);
    let state = RssState {
        url: url.to_string(),
        send_to: id,
        extract_content: ExtractContent::Raw,
        last_post: None,
        unique_by: UniqueBy::Link,
    };
    state.save(&format!("{}-{}.json", id, pin));
    format!(
        "Added {url} with pin {pin}. Type /status {pin} for more information.",
        url = url,
        pin = pin,
    )
}

fn status(sender: &User, id: &str) -> String {
    let pin = match id.parse() {
        Ok(pin) => pin,
        Err(_e) => return "Usage: /status <PIN>\nYou can get the PIN by typing /list".to_string(),
    };

    status_pin(sender.id.into(), pin)
}

fn status_pin(user_id: i64, pin: u16) -> String {
    let file_name = format!("{}-{}.json", user_id, pin);
    if let Some(state) = RssState::load(&file_name) {
        format!(
            "[{pin}] {url}\n - unique by {unique_by:?}\n - {content}\n",
            pin = pin,
            url = state.url,
            unique_by = state.unique_by,
            content = state.extract_content.describe()
        )
    } else {
        "Not found".to_string()
    }
}

async fn list(sender: &User) -> String {
    let mut result = String::new();
    let sender_id: i64 = sender.id.into();
    println!("Listing feeds of user {}", sender_id);
    for file in read_dir("sources").unwrap() {
        let file = file.unwrap();
        let file_name = file.file_name();
        let file_name = file_name.to_str().unwrap();
        println!("{:?}", file_name);
        if let Some((user_id, pin)) = data::get_user_id_and_pin_from_name(file_name) {
            println!("  {} {}", user_id, pin);
            if sender_id == user_id {
                result += &status_pin(user_id, pin);
            }
        }
    }
    result += &format!(
        "Checking for updates in {:?}",
        background::time_until_next_update().await
    );
    result
}

async fn edit(sender: &User, right: &str) -> String {
    fn edit_args(right: &str) -> Option<(u16, &str, &str)> {
        let mut split = right.splitn(3, ' ');
        let id = split.next()?;
        let id = id.parse().ok()?;
        let sub = split.next()?;
        let args = split.next()?;
        Some((id, sub, args))
    }
    let (pin, sub, args) = match edit_args(right) {
        Some(v) => v,
        None => return "Usage: /edit <PIN> <unique|content> <args>".to_string(),
    };
    let user_id: i64 = sender.id.into();
    let file_name = format!("{}-{}.json", user_id, pin);

    let mut state = match RssState::load(&file_name) {
        Some(state) => state,
        None => return "PIN not found. You can list all your feeds by typing /list".to_string(),
    };

    match (sub, args) {
        ("unique", "link") => {
            state.unique_by = UniqueBy::Link;
        }
        ("unique", "guid") => {
            state.unique_by = UniqueBy::Guid;
        }
        ("unique", _) => {
            return "Usage: /edit <PIN> unique <link|guid>".to_string();
        }
        ("content", "raw") => {
            state.extract_content = ExtractContent::Raw;
        }
        ("content", "find image") => {
            state.extract_content = ExtractContent::FindImage;
        }
        ("content", _) => {
            return "Usage: /edit <PIN> content <raw|find image>".to_string();
        }
        (_, _) => {
            return "Usage: /edit <PIN> <unique|content> <args>".to_string();
        }
    }

    state.save(&file_name);
    format!(
        "Saved! Next update will be in {:?}",
        background::time_until_next_update().await
    )
}

fn delete(_sender: &User, _id: &str) -> String {
    "Not implemented".to_string()
}

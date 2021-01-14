use futures::stream::StreamExt;
use rand::{thread_rng, Rng};
use std::fs::read_dir;
use telegram_bot::{Api, MessageKind, SendMessage, UpdateKind, User};

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
    let right = right.trim();
    /* commands to botfather:
    add - Add a new rss feed. Will return a random PIN number.
    list - List all RSS feeds and their PINs
    status - Get the status of the RSS feed
    show - Get the last post of the RSS feed
    edit - Edit how the RSS feed is handled
    delete - Delete the RSS feed
    */
    match left.to_ascii_lowercase().as_str() {
        "/start" => "Hello! Add rss feeds by typing /add <url>".to_owned(),
        "/add" => add(sender, right),
        "/status" => status(sender, right).await,
        "/list" => list(sender).await,
        "/edit" => edit(sender, right).await,
        "/delete" => delete(sender, right),
        "/show" => show(sender, right).await,
        "/source" | "/github" => "https://github.com/VictorKoenders/rss_telegram_bot".to_owned(),
        _ => "Unknown command.\nyou can list your rss feeds by typing /list.\nYou can add a new rss feed by typing /add <RSS url>".to_owned()
    }
}

fn add(sender: &User, url: &str) -> String {
    let id = sender.id.into();
    let pin = thread_rng().gen_range(1111..=9999);
    let state = RssState {
        url: url.to_string(),
        send_to: id,
        extract_content: ExtractContent::Raw,
        last_post: None,
        unique_by: UniqueBy::Link,
    };
    state.save(id, pin);
    format!(
        "Added {url} with pin {pin}. Type /status {pin} for more information.",
        url = url,
        pin = pin,
    )
}

async fn status(sender: &User, id: &str) -> String {
    let pin = match id.parse() {
        Ok(pin) => pin,
        Err(_e) => return "Usage: /status <PIN>\nYou can get the PIN by typing /list".to_string(),
    };

    let mut status = status_pin(sender.id.into(), pin);
    status += &format!(
        "\nNext update in {}",
        background::time_until_next_update().await
    );
    status
}

fn status_pin(user_id: i64, pin: u16) -> String {
    if let Some(state) = RssState::load(user_id, pin) {
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
    for file in read_dir("sources").unwrap() {
        let file = file.unwrap();
        let file_name = file.file_name();
        let file_name = file_name.to_str().unwrap();
        if let Some((user_id, pin)) = data::get_user_id_and_pin_from_name(file_name) {
            if sender_id == user_id {
                result += &status_pin(user_id, pin);
            }
        }
    }
    result += &format!(
        "Checking for updates in {}",
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

    let mut state = match RssState::load(user_id, pin) {
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

    state.save(user_id, pin);
    format!(
        "Saved! Next update will be in {}",
        background::time_until_next_update().await
    )
}

fn delete(sender: &User, id: &str) -> String {
    let pin: u16 = match id.parse() {
        Ok(pin) => pin,
        Err(_) => return "Usage: /delete <PIN>".to_string(),
    };
    let user_id: i64 = sender.id.into();
    if let Some(state) = RssState::load(user_id, pin) {
        state.delete(user_id, pin);
        format!(
            "Deleted [{pin}] {url}. You won't get any more notifications.",
            pin = pin,
            url = state.url
        )
    } else {
        "PIN not found. List all your feeds by typing /list".to_string()
    }
}

async fn show(sender: &User, id: &str) -> String {
    let user_id: i64 = sender.id.into();
    let pin: u16 = match id.parse() {
        Ok(pin) => pin,
        Err(_) => return "Usage: /delete <PIN>".to_string(),
    };
    if let Some(state) = RssState::load(user_id, pin) {
        match background::get_last_post(&state).await {
            Ok(last_notification) => last_notification.format(pin, &state),
            Err(e) => {
                eprintln!("Could not load RSS feed {url}", url = state.url);
                eprintln!("{:?}", e);
                format!(
                    "Could not RSS feed at {url}. Is the server available?",
                    url = state.url
                )
            }
        }
    } else {
        "PIN not found. List all your feeds by typing /list".to_string()
    }
}

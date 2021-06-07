use crate::{RssNotification, RssState};
use lazy_static::lazy_static;
use std::time::{Duration, Instant};
use telegram_bot::{Api, SendMessage, UserId};
use tokio::sync::RwLock;

pub fn spawn(token: String) {
    tokio::spawn(notifier(token));
}

lazy_static! {
    static ref LAST_UPDATE_TIME: RwLock<Instant> = RwLock::new(Instant::now());
}

const NOTIFY_INTERVAL: Duration = Duration::from_secs(3600);

pub async fn time_until_next_update() -> String {
    let elapsed = LAST_UPDATE_TIME.read().await.elapsed();
    let time = NOTIFY_INTERVAL - elapsed;
    if time > Duration::from_secs(60) {
        format!("{} minutes", time.as_secs() / 60)
    } else {
        format!("{} seconds", time.as_secs())
    }
}

async fn notifier(token: String) {
    loop {
        let api = Api::new(&token);

        for name in std::fs::read_dir("sources").unwrap() {
            let name = name.unwrap().file_name();
            let name = name.to_str().unwrap();
            let (user_id, pin) = match crate::data::get_user_id_and_pin_from_name(name) {
                Some(v) => v,
                None => {
                    println!(
                        "Warning: Could not load user id and pin from name {:?}",
                        name
                    );
                    continue;
                }
            };

            let mut state = RssState::load(user_id, pin).unwrap();
            let notifications = match get_rss(&mut state).await {
                Ok(notifications) => notifications,
                Err(e) => {
                    eprintln!("{}-{}.json - Could not load RSS feed at {}", user_id, pin, state.url);
                    eprintln!("{:?}", e);
                    continue;
                }
            };

            for notification in notifications {
                let message = notification.format(pin, &state);
                if let Err(e) = api
                    .send(SendMessage::new(UserId::new(state.send_to), message))
                    .await
                {
                    eprintln!("Could not send notification to user {}", state.send_to);
                    eprintln!("{:?}", e);
                }
            }
            state.save(user_id, pin);
        }

        *(LAST_UPDATE_TIME.write().await) = Instant::now();

        tokio::time::sleep(NOTIFY_INTERVAL).await;
    }
}

async fn get_rss(state: &mut RssState) -> Result<Vec<RssNotification>, Box<dyn std::error::Error>> {
    let bytes = reqwest::get(&state.url).await?.bytes().await?;
    let rss = rss::Channel::read_from(&bytes[..])?;

    let mut result = Vec::new();
    if let Some(previous_last_post) = state.last_post.as_ref() {
        for item in &rss.items {
            let unique_token = state.unique_by.get_value(item);
            if previous_last_post == unique_token {
                break;
            }
            result.push(RssNotification::new(item));
        }
    }
    if let Some(first) = rss.items.first() {
        let unique_token = state.unique_by.get_value(first);
        state.last_post = Some(unique_token.to_string());
    }

    Ok(result)
}

pub async fn get_last_post(
    state: &RssState,
) -> Result<RssNotification, Box<dyn std::error::Error>> {
    let bytes = reqwest::get(&state.url).await?.bytes().await?;
    let rss = rss::Channel::read_from(&bytes[..])?;
    let post = rss
        .items
        .first()
        .ok_or_else(|| String::from("No posts found"))?;
    Ok(RssNotification::new(&post))
}

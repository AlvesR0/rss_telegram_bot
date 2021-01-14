use lazy_static::lazy_static;
use std::time::{Duration, Instant};
use telegram_bot::;
use tokio::sync::RwLock;

pub fn spawn(token: String) {
    tokio::spawn(notifier(token));
}

lazy_static! {
    static ref LAST_UPDATE_TIME: RwLock<Instant> = RwLock::new(Instant::now());
}

const NOTIFY_INTERVAL: Duration = Duration::from_secs(3600);

pub async fn time_until_next_update() -> Duration {
    let elapsed = LAST_UPDATE_TIME.read().await.elapsed();
    NOTIFY_INTERVAL - elapsed
}

async fn notifier(token: String) {
    loop {
        let api = Api::new(&token);

        for name in std::fs::read_dir("sources").unwrap() {
            let name = name.unwrap().file_name();
            let name = name.to_str().unwrap();
            let (_user_id, pin): (i64, i32) = if let Some(name) = name.strip_suffix(".json") {
                let mut split = name.split('-');
                if let (Some(user_id), Some(pin)) = (
                    split.next().and_then(|id| id.parse().ok()),
                    split.next().and_then(|id| id.parse().ok()),
                ) {
                    (user_id, pin)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            let mut state = RssState::load(name).unwrap();
            let notifications = get_rss(&mut state).await.unwrap();

            for notification in notifications {
                let content = state
                    .extract_content
                    .extract(&notification.content)
                    .unwrap_or(&notification.content);
                let message = format!(
                    "[{pin}] {title}\n{content}\n{url}",
                    pin = pin,
                    title = notification.title,
                    content = content,
                    url = notification.url
                );
                api.send(SendMessage::new(UserId::new(state.send_to), message))
                    .await
                    .unwrap();
            }
            state.save(name);
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

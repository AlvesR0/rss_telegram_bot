use super::RssState;

#[derive(Debug)]
pub struct RssNotification {
    pub title: String,
    pub url: String,
    pub content: String,
}

impl RssNotification {
    pub fn new(item: &rss::Item) -> Self {
        Self {
            title: item.title.clone().unwrap_or_else(String::new),
            url: item.link.clone().unwrap_or_else(String::new),
            content: item.description.clone().unwrap_or_else(String::new),
        }
    }

    pub fn format(&self, pin: u16, state: &RssState) -> String {
        let mut content = state
            .extract_content
            .extract(&self.content)
            .unwrap_or(&self.content)
            .to_owned();
        if content.len() > 1024 {
            content = format!("{} [..]", &content[..1024]);
        }
        format!(
            "[{pin}] {title}\n{content}\n{url}",
            pin = pin,
            title = self.title,
            content = content,
            url = self.url
        )
    }
}

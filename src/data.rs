use std::fs::File;

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
}

pub fn get_user_id_and_pin_from_name(name: &str) -> Option<(i64, u16)> {
    let prefix = name.strip_suffix(".json")?;
    let split_index = prefix.chars().position(|c| c == '-')?;
    let (user_id, pin) = prefix.split_at(split_index);
    let user_id = user_id.parse().ok()?;
    let pin = pin[1..].parse().ok()?;
    Some((user_id, pin))
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct RssState {
    pub url: String,
    pub unique_by: UniqueBy,
    pub extract_content: ExtractContent,
    pub last_post: Option<String>,
    pub send_to: i64,
}

#[derive(Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Debug)]
pub enum UniqueBy {
    Guid,
    Link,
}

#[derive(Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Debug)]
pub enum ExtractContent {
    Raw,
    FindImage,
}

impl ExtractContent {
    pub fn extract<'a>(&self, content: &'a str) -> Option<&'a str> {
        match self {
            ExtractContent::Raw => Some(content),
            ExtractContent::FindImage => {
                let key = "img src=\"";
                let index = content.find(key)?;
                let index = index + key.len();
                let len = content[index..].find('"')?;
                Some(&content[index..index + len])
            }
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            ExtractContent::Raw => "not parsing content",
            ExtractContent::FindImage => "showing first image",
        }
    }
}

impl UniqueBy {
    pub fn get_value<'a>(&self, item: &'a rss::Item) -> &'a str {
        match self {
            UniqueBy::Guid => item
                .guid
                .as_ref()
                .map(|g| g.value.as_str())
                .unwrap_or("unknown"),
            UniqueBy::Link => item.link.as_deref().unwrap_or("unknown"),
        }
    }
}

impl RssState {
    pub fn load(filename: &str) -> Option<Self> {
        let mut file = File::open(format!("sources/{}", filename)).ok()?;
        serde_json::from_reader(&mut file).ok()
    }

    pub fn save(&self, filename: &str) {
        let mut file = File::create(format!("sources/{}", filename)).unwrap();
        serde_json::to_writer_pretty(&mut file, &self).unwrap();
    }
}

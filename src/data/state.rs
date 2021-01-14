use super::{ExtractContent, UniqueBy};
use std::fs::{remove_file, File};

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct RssState {
    pub url: String,
    pub unique_by: UniqueBy,
    pub extract_content: ExtractContent,
    pub last_post: Option<String>,
    pub send_to: i64,
}

impl RssState {
    fn file_path(sender_id: i64, pin: u16) -> String {
        format!("sources/{}-{}.json", sender_id, pin)
    }

    pub fn load(sender_id: i64, pin: u16) -> Option<Self> {
        let name = Self::file_path(sender_id, pin);
        let mut file = File::open(&name).ok()?;
        serde_json::from_reader(&mut file).ok()
    }

    pub fn delete(&self, sender_id: i64, pin: u16) {
        let name = Self::file_path(sender_id, pin);
        if let Err(e) = remove_file(&name) {
            eprintln!("Could not delete feed {:?}", name);
            eprintln!("{:?}", e);
        }
    }

    pub fn save(&self, sender_id: i64, pin: u16) {
        let name = Self::file_path(sender_id, pin);
        let mut file = File::create(&name).unwrap();
        serde_json::to_writer_pretty(&mut file, &self).unwrap();
    }
}

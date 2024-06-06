use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use scraper::Selector;
use tracing::warn;

use super::Identifier;


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct UNMember {
    pub name: String,
    pub iso_3166: Option<Identifier>,
}

impl Display for UNMember {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        match self.iso_3166 {
            Some(ref c) => write!(f, "{} ({})", self.name, c),
            None => write!(f, "{}", self.name),
        }
    }
}

impl UNMember {
    pub fn new(name: String, code: Option<Identifier>) -> Self {
        Self { name, iso_3166: code }
    }
    pub async fn fetch_un_nations(url: &str, countries: &BTreeMap<Identifier, Vec<String>>) -> Result<Vec<Self>> {
        let html = crate::fetch::get_html(url).await?;
        let selector = Selector::parse(".country div>h2").unwrap();
        let mut nations = Vec::new();
    
        for e in html.select(&selector) {
            let title = e.text().collect::<String>();
            let id = countries.iter()
                .find(|(_, v)|v.contains(&title.trim().to_string()))
                .map(|(k, _)|k.to_owned());

            if id.is_none() {
                warn!("Unable to match UN member {} with input country", title);
            }
            
            nations.push(Self::new(title, id));
        }
    
        if nations.is_empty() {
            bail!("Failed to fetch UN member states");
        }
    
        Ok(nations)
    }
}

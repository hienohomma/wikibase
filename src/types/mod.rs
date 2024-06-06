mod sovereign_state;
mod region;
mod flag;
mod un_nations;
mod currency;
mod calling_codes;
mod language;
mod capital;

use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;

use serde::{Deserialize, Serialize};
use scraper::ElementRef;

pub use sovereign_state::SovereignState;
pub use region::Region;
pub use flag::Flag;
pub use un_nations::UNMember;
pub use currency::Currency;
pub use calling_codes::CallingCode;
pub use language::Language;
pub use capital::Capital;


#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Identifier (
    #[serde(deserialize_with = "Identifier::deserialize", serialize_with = "Identifier::serialize")]
    pub String
);

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.0)
    }
}

impl Identifier {
    pub fn new(exonym: &str) -> Self {
        // Exclude leading and trailing whitespace, convert to lowercase
        let lc = exonym.trim().to_lowercase();

        // If comma is present lets cut from there
        let id = lc.split(",").next().unwrap_or(&lc);


        // Replace spaces with underscores
        let spaceless = id.replace(" ", "_");


        // Replace dashes with underscores
        let id = spaceless.replace("-", "_");

        Self(id.to_owned())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    fn serialize<S>(id: &String, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let lid = id.to_lowercase();
        serializer.serialize_str(&lid)
    }
    fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(s.to_lowercase())
    }
}


fn link_title_if(prefix: &str, elref: ElementRef) -> Option<String> {
    let s = elref.attr("href").unwrap_or("");

    if !s.starts_with(prefix) {
        return None
    }

    elref.attr("title").and_then(|s|Some(s.trim().to_string()))
}

fn link_text_if(prefix: &str, elref: ElementRef) -> Option<String> {
    let s = elref.attr("href").unwrap_or("");

    if !s.starts_with(prefix) {
        return None
    }

    for s in elref.text() {
        let t = s.trim();

        if t.is_empty() {
            continue;
        }

        return Some(t.to_owned())
    }
    
    None
}

fn link_title_and_text_opt_if(prefix: &str, elrefs: &Vec<ElementRef>) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut text = None;

    for e in elrefs {
        if title.is_none() {
            title = link_title_if(prefix, *e);
        }

        if text.is_none() {
            text = link_text_if(prefix, *e);
        }

        if title.is_some() && text.is_some() {
            break;
        }
    }

    (title, text)
}

fn inner_text_first_if(min: usize, max: Option<usize>, inner: &Vec<String>) -> Option<String> {
    for i in inner {
        let i = i.trim();

        if i.len() < min {
            continue;
        }

        if let Some(u) = max {
            if i.len() > u {
                continue;
            }
        }

        return Some(i.to_owned())
    }

    None
}

// fn inner_text_last_if(min: usize, max: Option<usize>, inner: &Vec<String>) -> Option<String> {
//     let mut inner = inner.clone();
//     inner.reverse();

//     inner_text_first_if(min, max, &inner)
// }

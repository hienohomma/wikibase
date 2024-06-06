use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;
use anyhow::{anyhow, bail, Result};
use scraper::Html;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::map::{Include, Found, map_from_table_data, Select};
use crate::types::link_text_if;

use super::{link_title_if, Identifier, UNMember};


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct SovereignState {
    pub name_short: String,
    pub name_long: String,
    pub un_member: bool,
    pub disputed: bool,
}

impl Display for SovereignState {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.name_long)
    }
}

impl SovereignState {
    pub fn new(name_short: String, name_long: String, un_member: bool, disputed: bool) -> Self {
        Self {
            name_short,
            name_long,
            un_member,
            disputed
        }
    }
    pub fn from_html(html: &Html, un_nations: &Vec<UNMember>, countries: &BTreeMap<Identifier, Vec<String>>) -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::Matching("a")));
        cols.insert(1, Some(Select::InnerAsText));
        cols.insert(2, Some(Select::InnerAsText));
        cols.insert(3, None);

        let collect = Include::Some { th_count: 4, td_map: cols };
        let mut items = BTreeMap::new();
    
        for m in map_from_table_data(html, collect, None)? {
            let (name,  name_long) = match m.get(&0).unwrap() {
                Found::Children(c) => c.iter()
                    .find_map(|e|link_title_if("/wiki/", *e)
                        .and_then(|n|link_text_if("/wiki/", *e)
                        .and_then(|t|Some((t.trim().to_string(), n))
                    ))
                )
                .ok_or(anyhow!("Expected to find a link with country name"))?,
                _ => bail!("Expected elements for country name column")
            };

            // Take country identifier from UN members list (2 letter ISO 3166) if it exists
            let mut id = un_nations.iter()
                .find_map(|n|match n.iso_3166.is_some() && (
                    n.name.to_lowercase() == name.to_lowercase() ||
                    n.name.to_lowercase() == name_long.to_lowercase()
                ) {
                    true => n.iso_3166.to_owned(),
                    false => None
                });

            // If UN list didn't give us a country code, try to look from the input countries list
            if id.is_none() {
                id = countries.iter()
                    .find(|(_, v)|v.iter().find(|n|
                        n.to_lowercase() == name.to_lowercase() || n.to_lowercase() == name_long.to_lowercase()
                    ).is_some())
                    .map(|(k, _)|k.to_owned());
            }

            // As a last resort just create identifier (not ISO 3166) from the name
            if id.is_none() {
                id = Some(Identifier::new(&name));
            }

            // Unwrap the identifier, now we're sure it exists
            let id = id.unwrap();

            // Remove everything that comes after comma. Leave the long version intact.
            let name_short = name.split(",").next().unwrap_or(&name).to_string();
    
            let member = match m.get(&1).unwrap() {
                Found::InnerText(c) => c.iter().any(|s|
                    s.to_lowercase().contains("un member state") && s.len() < 25
                ),
                _ => bail!("Expected content")
            };

            if !un_nations.is_empty() {
                match member {
                    true => {
                        if !un_nations.iter().any(|n|n.name.to_lowercase() == name.to_lowercase()) {
                            debug!("Data inconsistency in Wikipedia: According to UN list {} is not a UN member state", name_long);
                        }
                    },
                    false => {
                        if un_nations.iter().any(|n|n.name.to_lowercase() == name.to_lowercase()) {
                            debug!("Data inconsistency: Wikipedia thinks {} is not a UN member state but UN list has it", name_long);
                        }
                    }
                
                }
            }
    
            let dispute = match m.get(&2).unwrap() {
                Found::InnerText(c) => !c.iter().any(|s|s.to_lowercase().contains("none")),
                _ => bail!("Expected content")
            };
    
            if member && dispute {
                warn!("{} is a UN member state but has a dispute", name_long);
            }

            if items.contains_key(&id) {
                warn!("{} already exists, skipping", name_long);
                continue;
            }
    
            items.insert(id, SovereignState::new(name_short, name_long, member, dispute));
        }
    
        Ok(items)
    }
}

// pub fn sovereign_state_by_opt(countries: &Vec<SovereignState>, first: Option<String>, second: Option<String>) -> Result<SovereignState> {
//     if let Some(c) = &first {
//         if let Some(s) = countries.iter().find(|s|s.name_short.to_lowercase() == c.trim().to_lowercase()) {
//             return Ok(s.to_owned())
//         }
//     }

//     if let Some(c) = &second {
//         if let Some(s) = countries.iter().find(|s|s.name_short.to_lowercase() == c.trim().to_lowercase()) {
//             return Ok(s.to_owned())
//         }
//     }

//     if first.is_some() && second.is_some() {
//         bail!("Country not found from provided list with name {} or {}", first.unwrap(), second.unwrap())
//     }

//     else if first.is_some() {
//         bail!("Country not found from provided list with name {}", first.unwrap())
//     }

//     else if second.is_some() {
//         bail!("Country not found from provided list with name {}", second.unwrap())
//     }

//     else {
//         panic!("Hard to find a country if you don't provide search terms")
//     }
// }
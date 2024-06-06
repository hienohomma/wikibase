use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;
use anyhow::{bail, Result};
use scraper::Html;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::map::{Include, Found, map_from_table_data, Select};
use crate::types::region::region_by_opt;
use crate::types::{link_text_if, link_title_and_text_opt_if};

use super::{Identifier, Region};


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Capital {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endonyms: Option<Vec<String>>,
}

impl Display for Capital {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        match &self.endonyms {
            Some(e) => write!(f, "{} ({})", self.name, e.join(", ")),
            None => write!(f, "{}", self.name)
        }
    }
}

impl Capital {
    pub fn new(name: String, endonyms: Option<Vec<String>>) -> Self {
        Self {
            name,
            endonyms
        }
    }
    pub fn from_html(html: &Html, regions: &BTreeMap<Identifier, Region>, countries: Option<&BTreeMap<Identifier, Vec<String>>>)
    -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::Matching("a")));
        cols.insert(1, Some(Select::Matching("a")));
        cols.insert(2, None);
        cols.insert(3, Some(Select::Matching("span[lang]")));
        cols.insert(4, None); 

        let collect = Include::Some { th_count: 5, td_map: cols };
        let mut items: BTreeMap<Identifier, Capital> = BTreeMap::new();
    
        for m in map_from_table_data(html, collect, None)? {
            // Read country name
            let (reg_title, reg_text) = match m.get(&0) {
                Some(i) => match i {
                    Found::Children(v) => link_title_and_text_opt_if("/wiki/", v),
                    _ => bail!("Expected TD element children for country column"),
                },
                None => bail!("Expected country column"),
            };

            debug!("Processing capital of {:?} ({:?})", reg_title, reg_text);

            // Find the country in the map of regions
            let (iso_id, region) = match region_by_opt(regions, countries, reg_title.as_ref(), reg_text.as_ref()) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Skipping capital for {:?} / {:?}: {}", reg_text, reg_title, e);
                    continue;
                }
            };

            debug!("Found capital for {}", region);

            // Read capital name exonym
            let name = match m.get(&1) {
                Some(i) => match i {
                    Found::Children(v) => match v.into_iter().find_map(|i|link_text_if("/wiki/", *i)) {
                        Some(s) => s,
                        None => {
                            debug!("Expected link text for capital name of {}, failed to extract link from {} html elements", iso_id, v.len());
                            continue;
                        },
                    },
                    _ => bail!("Expected TD element children for capital column"),
                },
                None => bail!("Expected capital column"),
            };

            // Read capital name endonyms if any
            let endonym = match m.get(&3) {
                Some(i) => match i {
                    Found::Children(v) => v.iter()
                        .filter_map(|s|
                            // Remove the capital name from the list of endonyms if its the same as exonym
                            s.text().next().and_then(|s|match name.eq(s.trim()) {
                                true => None,
                                false => Some(s.trim().to_owned())
                            })
                        ).collect::<Vec<String>>(),
                    _ => bail!("Expected inner text for {} capital endonym cell", iso_id),
                },
                None => bail!("Expected {} capital endonym column", iso_id),
            };

            let endonyms = match endonym.len() {
                0 => None,
                _ => Some(endonym),
            };

            // Create new capital
            items.insert(iso_id, Capital::new(name, endonyms));
        }

        Ok(items)
    }
}

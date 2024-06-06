use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;
use anyhow::{bail, Result};
use scraper::Html;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::map::{Include, Found, map_from_table_data, Select};
use crate::types::region::region_by_opt;
use crate::types::link_text_if;

use super::{Identifier, Region};


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct CallingCode (pub String);

impl Display for CallingCode {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.0)
    }
}

impl CallingCode {
    pub fn new(code: String) -> Self {
        Self(code)
    }
    pub fn from_html(html: &Html, iso_3166: &BTreeMap<Identifier, Region>, countries: Option<&BTreeMap<Identifier, Vec<String>>>)
    -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::TdElement)); // country where used
        cols.insert(1, Some(Select::Matching("a"))); // code
        cols.insert(2, None);
        cols.insert(3, None);

        let collect = Include::Some { th_count: 5, td_map: cols };
        let mut items: BTreeMap<Identifier, CallingCode> = BTreeMap::new();
    
        for m in map_from_table_data(html, collect, None)? {
            // Read the calling code first
            let code = match m.get(&1) {
                Some(i) => match i {
                    Found::Children(v) => match v.into_iter().find_map(|e|link_text_if("/wiki/", *e)) {
                        Some(c) => c,
                        None => {
                            debug!("Expected calling code link text (most likely another table with same column count)");
                            continue;
                        },
                    },
                    _ => bail!("Expected link element in calling code column"),
                },
                None => bail!("Expected calling code column"),
            };

            // Read in which country the code serves
            let vals = match m.get(&0) {
                Some(i) => match i {
                    Found::Parent(e) => e.text().into_iter().take(2).map(|s|s.trim().to_string()).collect::<Vec<String>>(),
                    _ => bail!("Expected TD element children for country column"),
                },
                None => bail!("Expected country column"),
            };

            debug!("Processing calling code {} of {:?} ({:?})", code, vals.get(0), vals.get(1));

            // Find the iso 3166 identifier for the country
            let iso_id = match region_by_opt(iso_3166, countries, vals.get(0), vals.get(1)) {
                Ok(c) => c.0,
                Err(e) => {
                    warn!("Skipping calling code {}: {}", code, e);
                    continue;
                }
            };

            if items.contains_key(&iso_id) {
                warn!("Skipping calling code {}: Duplicate entry", iso_id);
                continue;
            }

            items.insert(iso_id, CallingCode::new(code));
        }
    
        Ok(items)
    }
}

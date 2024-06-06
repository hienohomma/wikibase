use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;

use anyhow::{anyhow, bail, Result};
use scraper::Html;
use serde::{Serialize, Deserialize};
use scraper::Selector;
use tracing::{debug, warn};

use crate::map::{Include, Found, map_from_table_data, Select};

use super::{link_text_if, link_title_if, Identifier, SovereignState};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso3166_1 {
    pub a2: String,
    pub a3: String,
    pub num: u16,
}

impl Display for Iso3166_1 {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.a2)
    }
}

impl Iso3166_1 {
    pub fn new(a2: String, a3: String, num: u16) -> Result<Self> {
        if a2.len() != 2 {
            bail!("Expected 2 characters for a2, got {}", a2.len());
        }

        if a3.len() != 3 {
            bail!("Expected 3 characters for a3, got {}", a3.len());
        }

        Ok(Self {
            a2,
            a3,
            num
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso3166_2 (pub String);

impl Display for Iso3166_2 {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.0)
    }
}

impl Iso3166_2 {
    pub fn new(s: String) -> Result<Self> {
        let clean = s.trim().to_uppercase();

        // Expected: ISO 3166-2:XX
        match clean.starts_with("ISO 3166-2:") && clean.len() == 13 {
            true => Ok(Self(clean)),
            false => bail!("Expected ISO 3166-2: prefix, got {}", clean),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tld (pub Vec<String>);

impl Display for Tld {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        match self.0.len() {
            1 => write!(f, "{}", self.0.first().unwrap()),
            0 => write!(f, "[not implemented]"),
            _ => write!(f, "{}", self.0.join(", "))
        }
        
    }
}

impl Tld {
    pub fn new(v: Vec<String>) -> Result<Self> {
        let mut valid = vec![];

        for s in v {
            let clean = s.trim().to_lowercase();
    
            // Expected: .xx
            match clean.starts_with(".") && clean.len() == 3 {
                true => valid.push(clean),
                false => bail!("Expected .xx domain tld, got {}", clean),
            }
        }

        Ok(Self(valid))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub state_name: String,
    pub un_member: bool,
    pub sovereignity: Identifier,
    pub iso_3166_1: Iso3166_1,
    pub iso_3166_2: Iso3166_2,
    pub tld: Tld
}

impl Display for Region {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.name)
    }
}

impl Region {
    pub fn new(name: String, state_name: String, sovereignity: Identifier, un_member: bool, iso_3166_1: Iso3166_1, iso_3166_2: Iso3166_2, tld: Tld) -> Self {
        Self {
            name,
            state_name,
            un_member,
            sovereignity,
            iso_3166_1,
            iso_3166_2,
            tld
        }
    }
    pub fn from_html(html: &Html, sovereign_states: &BTreeMap<Identifier, SovereignState>) -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Select::Matching("a"));
        cols.insert(1, Select::Matching("a"));
        cols.insert(2, Select::TdElement);
        cols.insert(3, Select::Matching("a > span"));
        cols.insert(4, Select::Matching("a > span"));
        cols.insert(5, Select::Matching("a > span"));
        cols.insert(6, Select::Matching("a"));
        cols.insert(7, Select::Matching("a"));

        let collect = Include::All { th_count: 9, td_map: cols };
        let mut items = BTreeMap::new();
    
        for m in map_from_table_data(html, collect, None)? {
            let name = match m.get(&0).unwrap() {
                Found::Children(c) => c.iter()
                    .find_map(|e|link_title_if("/wiki/", *e))
                    .ok_or(anyhow!("Failed to read ISO 3166 name"))?,
                _ => bail!("Expected elements for region name")
            };

            // 3rd column is the charm here, sovereignity defines how to treat the entry
            let (iso_id, sovereignity, un_member) = match m.get(&2) {
                Some(v) => match v {
                    Found::Parent(v) => {
                        // Can be a link with a text 'UN member'
                        let member_link = v.select(&Selector::parse("a").unwrap())
                            .any(|e|e.text().any(|s|s.to_lowercase().contains("un member")));

                        // Can be a plain string 'UN member'
                        let member_str = v.text().any(|s|s.to_lowercase().contains("un member"));

                        // Can be a link to a sovereign state
                        let state_ref = v.select(&Selector::parse("a").unwrap())
                            .find_map(|e|link_text_if("#", e));

                        // Try to find by exonym first, then by name if that fails
                        if member_link || member_str {
                            match sovereign_states.iter().find(|(_, s)|s.name_short.to_lowercase() == name.to_lowercase()) {
                                Some(t) => (t.0.clone(), t.1.clone(), true),
                                None => match sovereign_states.iter().find(|(_, i)|i.name_long.to_lowercase() == name.to_lowercase()) {
                                    Some(t) => (t.0.clone(), t.1.clone(), true),
                                    None => {
                                        warn!("Failed to find ISO 3166 '{}' from provided list of sovereign states", name);
                                        continue;
                                    }
                                }
                            }
                        }
                        // As for the UN members above, search sovereignity reference with exonym and name
                        else if let Some(s) = state_ref {
                            match sovereign_states.iter().find(|(_, i)|i.name_short.to_lowercase() == s.to_lowercase()) {
                                Some(t) => (t.0.clone(), t.1.clone(), false),
                                None => match sovereign_states.iter().find(|(_, i)|i.name_long.to_lowercase() == s.to_lowercase()) {
                                    Some(t) => (t.0.clone(), t.1.clone(), false),
                                    None => {
                                        warn!("Failed to find ISO 3166 '{}' reference to '{}' from provided list of sovereign states", name, s);
                                        continue;
                                    }
                                }
                            }
                        }
                        else {
                            warn!("Failed to determine sovereignity for {}, skipping...", name);
                            continue;
                        }
                    },
                    _ => bail!("Expected parent element for sovereignity column for {}", name)
                },
                None => bail!("Failed to read sovereignity column for {}", name)
            };

            debug!("ISO 3166 region {} sovereignity set to {}", iso_id, sovereignity.name_long);

            // Get the official ISO 3166 country name from 2nd column
            let state_name = match m.get(&1).unwrap() {
                Found::Children(v) => {
                    let first = v.first().and_then(|e|e.text().next());

                    match first {
                        Some(s) => s.trim().to_string(),
                        None => bail!("Failed to read official state name for {}", name)
                    }
                },
                _ => bail!("Expected all for iso_3166 state name for {}", name)
            };

            let iso_3166_1_a2 = match m.get(&3).unwrap() {
                Found::Children(c) => match c.first() {
                    Some(e) => match e.text().next() {
                        Some(s) => {
                            let val = s.trim().to_uppercase();

                            match val.len() {
                                2 => val,
                                _ => bail!("Expected 2 characters for iso_3166_1 a2 ({}), got {}", name, val.len())
                            }
                        },
                        None => bail!("Expected text for iso_3166_1 a2")
                    },
                    None => bail!("Invalid ISO 3166-1 a2 code for {}", name)
                }
                _ => bail!("Expected elements for iso_3166_1 two letter code for {}", name)
            };

            let iso_3166_1_a3 = match m.get(&4).unwrap() {
                Found::Children(c) => match c.first() {
                    Some(e) => match e.text().next() {
                        Some(s) => {
                            let val = s.trim().to_uppercase();

                            match val.len() {
                                3 => val,
                                _ => bail!("Expected 3 characters for iso_3166_1 a3 ({}), got {}", name, val.len())
                            }
                        },
                        None => bail!("Expected text for iso_3166_1 a3 for {}", name)
                    },
                    None => bail!("Invalid ISO 3166-1 a3 code for {}", name)
                }
                _ => bail!("Expected elements for iso_3166_1 3 letter code for {}", name)
            };

            let iso_3166_1_num = match m.get(&5).unwrap() {
                Found::Children(c) => match c.first() {
                    Some(e) => match e.text().next() {
                        Some(s) => {
                            let val = s.trim().parse::<u16>()?;

                            match val {
                                0..=999 => val,
                                _ => bail!("Expected 3 digit number for iso_3166_1 num ({}), got {}", name, val)
                            }
                        },
                        None => bail!("Expected text for iso_3166_1 num for {}", name)
                    },
                    None => bail!("Invalid ISO 3166-1 num code for {}", name)
                }
                _ => bail!("Expected elements for iso_3166_1 numeric value for {}", name)
            };

            let iso_3166_2 = match m.get(&6).unwrap() {
                Found::Children(c) => c.iter()
                    .find_map(|e|link_title_if("/wiki/", *e))
                    .ok_or(anyhow!("Failed to read ISO 3166-2 column for {}", name))?,
                _ => bail!("Expected elements for iso_3166_2 value for {}", name)
            };

            let tld = match m.get(&7).unwrap() {
                Found::Children(c) => c.iter()
                    .filter_map(|e|link_title_if("/wiki/", *e))
                    .collect(),
                _ => bail!("Expected elements for tld value for {}", name)
            };

            // Create identifier from 2 letter code
            let id = Identifier::new(&iso_3166_1_a2);

            if items.contains_key(&id) {
                bail!("Duplicate entry for {} / {}", iso_3166_1_a2, name);
            }
    
            items.insert(
                id,
                Self::new( 
                    name,
                    state_name,
                    iso_id,
                    un_member,
                    Iso3166_1::new(iso_3166_1_a2, iso_3166_1_a3, iso_3166_1_num)?,
                    Iso3166_2::new(iso_3166_2)?,
                    Tld::new(tld)?
                )
            );
        }
    
        Ok(items)
    }
}

pub fn region_by_opt(
    regions: &BTreeMap<Identifier, Region>,
    countries: Option<&BTreeMap<Identifier, Vec<String>>>,
    first: Option<&String>,
    second: Option<&String>
) -> Result<(Identifier, Region)> {
    if let Some(t) = try_opt(first, countries, regions) {
        return Ok(t)
    }

    if let Some(t) = try_opt(second, countries, regions) {
        return Ok(t)
    }

    if first.is_some() && second.is_some() {
        bail!("ISO 3166 country not found from provided list with name {} or {}", first.unwrap(), second.unwrap())
    }

    else if first.is_some() {
        bail!("ISO 3166 country not found from provided list with name {}", first.unwrap())
    }

    else if second.is_some() {
        bail!("ISO 3166 country not found from provided list with name {}", second.unwrap())
    }

    else {
        panic!("Hard to find a ISO 3166 country if you don't provide search terms")
    }
}

fn try_opt(opt: Option<&String>, countries: Option<&BTreeMap<Identifier, Vec<String>>>, regions: &BTreeMap<Identifier, Region>) -> Option<(Identifier, Region)> {
    if let Some(c) = opt {
        if let Some((i, s)) = regions.iter().find(|(_, s)|
            s.name.to_lowercase() == c.trim().to_lowercase() ||
            s.state_name.to_lowercase() == c.trim().to_lowercase()
        ) {
            return Some((i.to_owned(), s.to_owned()))
        }

        // Try from input countries if provided
        if let Some(m) = countries {
            if let Some((i, _)) = m.iter().find(|(_, v)|v.iter().any(|b|b.to_lowercase() == c.trim().to_lowercase())) {
                if let Some(r) = regions.get(i) {
                    return Some((i.to_owned(), r.to_owned()))
                }
            }
        }
    }

    None
}
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;

use anyhow::{anyhow, bail, Result};
use scraper::{ElementRef, Html};
use serde::{Serialize, Deserialize};
use scraper::Selector;
use tracing::{debug, info, warn};

use crate::map::{Include, Found, map_from_table_data, Select};
use crate::types::link_title_and_text_opt_if;
use crate::types::region::region_by_opt;

use super::{link_text_if, link_title_if, Identifier, Region};

const EXCLUDE: [&str; 25] = [
    "has", "of", "de", "are", "in", "their", "they", "none", "and", "all", "have",
    "languages", "ethnic", "groups", "official", "territories", "facto",
    "status", "spoken", "another", "native", "wherever", "predominate", "autonomous",
    "republic"
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso639 {
    set1: String,
    set2_t: String,
    set2_b: String,
    set3: String,
}

impl Display for Iso639 {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.set1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub name_short: String,
    pub name_long: String,
    pub iso639: Iso639,
    #[serde(default)]
    pub regions: Vec<Identifier>,
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{}", self.name_short)
    }
}

impl Language {
    pub fn new(name_short: String, name_long: String, iso639: Iso639, regions: Option<Vec<Identifier>>) -> Self {
        Self {
            name_short,
            name_long,
            iso639,
            regions: match regions {
                Some(r) => r,
                None => Vec::new(),
            }
        }
    }
    pub fn from_html(html: &Html) -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::Matching("a")));
        cols.insert(1, Some(Select::Matching("a")));
        cols.insert(2, Some(Select::Matching("code")));
        cols.insert(3, Some(Select::Matching("code")));
        cols.insert(4, Some(Select::Matching("code")));
        cols.insert(5, None);

        let collect = Include::Some { th_count: 6, td_map: cols };
        let mut items = BTreeMap::new();

        for m in map_from_table_data(html, collect, None)? {
            // Name from the link title and text
            let (name_short, name_long) = match m.get(&0).unwrap() {
                Found::Children(c) => c.iter()
                    .find_map(|e|link_title_if("/wiki/", *e)
                        .and_then(|n|link_text_if("/wiki/", *e)
                        .and_then(|t|Some((t.trim().to_string(), n))
                    ))
                )
                .ok_or(anyhow!("Expected to find a link with language name"))?,
                _ => bail!("Expected elements for language name column")
            };

            // ISO 639 codes
            let set1 = match m.get(&1).unwrap() {
                Found::Children(c) => c.iter()
                    .find_map(|e|link_text_if("https://www.loc.gov/standards/iso639-2/", *e))
                    .ok_or(anyhow!("Expected to find a link with 2 letter language code for {}", name_long))?,
                _ => bail!("Expected elements for language 2 letter code column")
            };

            let set2_t: String = match m.get(&2).unwrap() {
                Found::Children(c) => c.iter()
                    .next()
                    .and_then(|e|Some(match e.select(&Selector::parse("b").unwrap()).next() {
                        Some(b) => b.text().collect(),
                        None => e.text().collect(),
                    }))
                    .ok_or(anyhow!("Expected to find a link with 3 letter set2/T language code for {}", name_long))?,
                _ => bail!("Expected elements for language 3 letter (set 2/T) code column")
            };

            let set2_b: String = match m.get(&3).unwrap() {
                Found::Children(c) => c.iter()
                    .next()
                    .and_then(|e|Some(match e.select(&Selector::parse("b").unwrap()).next() {
                        Some(b) => b.text().collect(),
                        None => e.text().collect(),
                    }))
                    .ok_or(anyhow!("Expected to find a link with 3 letter set2/B language code for {}", name_long))?,
                _ => bail!("Expected elements for language 3 letter (set 2/B) code column")
            };

            let set3: String = match m.get(&4).unwrap() {
                Found::Children(c) => c.iter()
                    .next()
                    .and_then(|e|Some(match e.select(&Selector::parse("b").unwrap()).next() {
                        Some(b) => b.text().collect(),
                        None => e.text().collect(),
                    }))
                    .ok_or(anyhow!("Expected to find a link with 3 letter set3 language code for {}", name_long))?,
                _ => bail!("Expected elements for language 3 letter (set 3) code column")
            };

            let id = Identifier::new(&set3);

            let iso639 = Iso639 {
                set1,
                set2_t,
                set2_b,
                set3,
            };

            items.insert(id, Self::new(name_short, name_long, iso639, None));
        }

        Ok(items)
    }
    pub fn zones_from_html(
        html: &Html,
        countries: &BTreeMap<Identifier, Vec<String>>,
        regions: &BTreeMap<Identifier, Region>,
        languages: &mut BTreeMap<Identifier, Self>
    ) -> Result<()> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::Matching("a")));
        cols.insert(1, Some(Select::TdElement));
        cols.insert(2, Some(Select::TdElement));
        cols.insert(3, None);
        cols.insert(4, None);
        cols.insert(5, None);

        let collect = Include::Some { th_count: 6, td_map: cols };

        for m in map_from_table_data(html, collect, None)? {
            // Read region where languages from next columns are used
            let (reg_title, reg_text) = match m.get(&0) {
                Some(i) => match i {
                    Found::Children(v) => link_title_and_text_opt_if("/wiki/", v),
                    _ => {
                        warn!("Expected TD element children for language region column");
                        continue;
                    },
                },
                None => bail!("Expected language country column to have at least one element"),
            };

            debug!("Processing language of {:?} ({:?})", reg_title, reg_text);

            // Find the country in the map of regions
            let (iso_id, region) = match region_by_opt(regions, Some(countries), reg_title.as_ref(), reg_text.as_ref()) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Skipping language region {:?} / {:?}: {}", reg_title, reg_text, e);
                    continue;
                }
            };

            debug!("Found region {} from language zones", region);

            // Read official language column which might hold one or many:
            // - link to language (internal wikipedia link)
            // - plain text language name inside <td> element
            // - plain text or wikipedia link with language name inside <li> element
            let official = match m.get(&1) {
                Some(i) => match i {
                    Found::Parent(e) => e,
                    _ => {
                        warn!("Expected TD element children for official language column");
                        continue;
                    },
                },
                None => {
                    warn!("Expected official language column to have at least one element");
                    continue;
                },
            };

            // Process official language cell data
            if let Err(e) = process_td_cell(official, languages, &iso_id) {
                warn!("Skipping official languages for {}: {}", iso_id, e);
                continue;
            }

            let regional = match m.get(&2) {
                Some(i) => match i {
                    Found::Parent(e) => e,
                    _ => {
                        warn!("Expected TD element children for regional language column");
                        continue;
                    },
                },
                None => {
                    warn!("Expected regional language column to have at least one element");
                    continue;
                },
            };

            // Process regional language cell data
            if let Err(e) = process_td_cell(regional, languages, &iso_id) {
                warn!("Skipping regional languages for {}: {}", iso_id, e);
                continue;
            }
        }
    
        Ok(())
    }
}

fn process_td_cell(td_e: &ElementRef, languages: &mut BTreeMap<Identifier, Language>, region: &Identifier) -> Result<()> {
    let mut items = Vec::new();

    // Test if we have a list of languages
    let li_sel = Selector::parse("li").unwrap();
    
    for li in td_e.select(&li_sel) {
        if let Some(l) = link_text_if("/wiki/", li) {
            items.push(l);

            // Try this too
            if let Some(l) = link_title_if("/wiki/", li) {
                items.push(l);
            }

            continue;
        }

        el_text_splitter(&li, &mut items);
    }

    // Proceed by checking if we have languages as links in a string
    let a_sel = Selector::parse("a").unwrap();

    for a in td_e.select(&a_sel) {
        if let Some(l) = link_text_if("/wiki/", a) {
            items.push(l);

            // Try this too
            if let Some(l) = link_title_if("/wiki/", a) {
                items.push(l);
            }
        }
    }

    // Might be a flat language name on the element or a novel of some sort containing language names here and there.
    // Lets just split from spaces and treat every word as a potential language.
    el_text_splitter(td_e, &mut items);

    // Compare found language names (or irrelevant crap) to known languages
    items.sort();
    items.dedup();

    for i in items.iter() {
        let lcl = i.to_lowercase();
        
        if let Some(l) = languages.values_mut().find(|l|l.name_short.to_lowercase() == lcl || l.name_long.to_lowercase() == lcl) {
            if l.regions.contains(&region) {
                debug!("Language {} already has region {}", i, region);
                continue;
            }

            info!("Added {} to language {}", region, &l.name_short);
            l.regions.push(region.to_owned());
        }
    }

    Ok(())
}

fn el_text_splitter(html_el: &ElementRef, items: &mut Vec<String>) {
    for t in html_el.text() {
        for s in t.split_whitespace() {
            // Only words with alphabetic characters are considered
            let mut word = String::new();
            
            for c in s.chars() {
                if c.is_alphabetic() {
                    word.push(c);
                }
            }

            // Exclude short words
            if word.len() < 3 {
                continue;
            }

            // See if word is present on the exclude list
            let s = word.to_lowercase();

            if EXCLUDE.contains(&s.as_str()) {
                continue;
            }
            
            items.push(word);
        }
    }
}
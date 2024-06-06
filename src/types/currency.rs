use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;
use anyhow::{bail, Result};
use scraper::Html;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::map::{Include, Found, map_from_table_data, Select};
use crate::types::region::region_by_opt;
use crate::types::{link_text_if, link_title_and_text_opt_if};

use super::{inner_text_first_if, Identifier, Region};


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Fraction {
    pub name: String,
    pub basic: u16,
}

impl Display for Fraction {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{} ({} to 1)", self.name, self.basic)
    }
}

impl Fraction {
    pub fn new(name: String, basic: u16) -> Self {
        Self {
            name,
            basic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Currency {
    pub name: String,
    pub symbol: String,
    pub fraction: Fraction,
    pub regions: Vec<Identifier>,
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{} ({})", self.name, self.symbol)
    }
}

impl Currency {
    pub fn new(name: String, symbol: String, fraction: Fraction, region: Option<Identifier>) -> Self {
        Self {
            name,
            symbol,
            fraction,
            regions: match region {
                Some(c) => vec![c],
                None => Vec::new(),
            },
        }
    }
    pub fn from_html(html: &Html, regions: &BTreeMap<Identifier, Region>, countries: Option<&BTreeMap<Identifier, Vec<String>>>)
    -> Result<BTreeMap<Identifier, Self>> {
        let mut cols = HashMap::new();
        cols.insert(0, Select::Matching("a")); // country where used
        cols.insert(1, Select::Matching("a")); // name
        cols.insert(2, Select::InnerAsText); // symbol
        cols.insert(3, Select::InnerAsText); // iso code <- IMPORTANT
        cols.insert(4, Select::Matching("a")); // fraction
        cols.insert(5, Select::InnerAsText); // fraction digits to form one complete

        let collect = Include::All { th_count: 6, td_map: cols };
        let mut items: BTreeMap<Identifier, Currency> = BTreeMap::new();
    
        for m in map_from_table_data(html, collect, None)? {
            // We collect each currency only once. Compare currency iso codes
            let iso = match m.get(&3) {
                Some(i) => match i {
                    Found::InnerText(v) => match v.into_iter().find(|s|s.trim().len() == 3) {
                        Some(s) => Identifier::new(s),
                        None => {
                            warn!("Skipping currency [{:?}] with invalid ISO code", v);
                            continue;
                        }
                    },
                    _ => bail!("Expected inner text for currency ISO code"),
                },
                None => {
                    warn!("Skipping currency without ISO code");
                    continue;
                }
            };

            // Read in which country the currency is used
            let (reg_title, reg_text) = match m.get(&0) {
                Some(i) => match i {
                    Found::Children(v) => link_title_and_text_opt_if("/wiki/", v),
                    _ => bail!("Expected TD element children for country column"),
                },
                None => bail!("Expected country column"),
            };

            debug!("Processing currency of {:?} ({:?})", reg_title, reg_text);

            // Find the country in the map of regions
            let (iso_id, region) = match region_by_opt(regions, countries, reg_title.as_ref(), reg_text.as_ref()) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Skipping currency {}: {}", iso, e);
                    continue;
                }
            };

            // See if we have this currency already and if so, just update it's list of countries where it circulates
            if let Some(c) = items.get_mut(&iso) {
                info!("Adding {} to the list of regions where currency {} circulates", region.name, c.name);
                
                c.regions.push(iso_id);
                continue;
            }

            // Read in the currency name
            let name = match m.get(&1) {
                Some(i) => match i {
                    Found::Children(v) => match v.into_iter().find_map(|e|link_text_if("/wiki/", *e)){
                        Some(s) => s,
                        None => {
                            warn!("Skipping currency of {} with invalid name", region.name);
                            continue;
                        },
                    },
                    _ => bail!("Expected link element for currency of {}", region.name),
                },
                None => {
                    warn!("Skipping currency of {} as it doesn't have a name", region.name);
                    continue;
                }
            };

            // Read in the currency symbol
            let symbol = match m.get(&2) {
                Some(i) => match i {
                    Found::InnerText(v) => match inner_text_first_if(1, None, v) {
                        Some(s) => match s.split(" ").next() {
                            Some(i) => i.to_string(),
                            None => s,
                        },
                        None => {
                            warn!("Skipping currency '{}' of {} with invalid symbol", name, region.name);
                            continue;
                        },
                    },
                    _ => bail!("Expected inner text for symbol of currency '{}' of {}", name, region.name),
                },
                None => {
                    warn!("Skipping currency '{}' of {} as it doesn't have a symbol",name, region.name);
                    continue;
                }
            };

            // Read in the fraction name
            let fraction_name = match m.get(&4) {
                Some(i) => match i {
                    Found::Children(v) => match v.into_iter().find_map(|e|link_text_if("/wiki/", *e)) {
                        Some(s) => s,
                        None => {
                            warn!("Skipping currency '{}' of {} with invalid fraction name", name, region.name);
                            continue;
                        },
                    },
                    _ => bail!("Expected link element for currency '{}' fraction of {}", name, region.name),
                },
                None => {
                    warn!("Skipping currency '{}' of {} as it doesn't have a fraction name", name, region.name);
                    continue;
                }
            };   

            // Read in the fraction basic
            let fraction_basic = match m.get(&5) {
                Some(i) => match i {
                    Found::InnerText(v) => match v.into_iter().find_map(|s|s.trim().parse::<u16>().ok()) {
                        Some(u) => u,
                        None => {
                            warn!("Skipping currency '{}' of {} with invalid fraction basic", name, region.name);
                            continue;
                        },
                    },
                    _ => bail!("Expected inner text for fraction units to basic for currency '{}' of {}", name, region.name),
                },
                None => {
                    warn!("Skipping currency '{}' of {} as it doesn't have a fraction basic", name, region.name);
                    continue;
                }
            };

            // Create new currency
            items.insert(iso.clone(), Currency::new(name, symbol, Fraction::new(fraction_name, fraction_basic), Some(iso_id)));
        }
    
        Ok(items)
    }
}

use std::collections::HashMap;
use anyhow::{anyhow, bail, Result};
use scraper::{ElementRef, Html, Selector};
use scraper::selectable::Selectable;
use tracing::{debug, info, warn};

pub enum Include {
    All{ th_count: usize, td_map: HashMap<usize, Select> },
    Some{ th_count: usize, td_map: HashMap<usize, Option<Select>>},
}

pub enum Select {
    Matching(&'static str),
    InnerAsText,
    TdElement
}

pub enum Found<'a> {
    Children(Vec<ElementRef<'a>>),
    InnerText(Vec<String>),
    Parent(ElementRef<'a>),
}

pub fn map_from_table_data<'a>(html: &'a Html, collect: Include, table_index_filter: Option<&[usize]>) -> Result<Vec<HashMap<usize, Found<'a>>>> {
    // Search for tables in html document
    let document = html.root_element();
    let table_sel = Selector::parse("table").unwrap();
    let doc_table_els = document.select(&table_sel).collect::<Vec<ElementRef<'_>>>();

    if doc_table_els.is_empty() {
        bail!("Provided html document does not contain any tables");
    }

    // We're only interested in TD elements, but we need to be aware of the columns count
    // so read TR elements first and see if TD element count inside matches column collect arg
    let tr_sel = Selector::parse("tr").unwrap();
    let td_sel = Selector::parse("td").unwrap();
    let th_sel = Selector::parse("th").unwrap();
    
    // Loop through tables, match 
    let mut elements = Vec::new();

    for (table_i, table_el) in doc_table_els.iter().enumerate() {
        // Skip table if only tables from given indexes were requested and this is not on the list
        if let Some(r) = table_index_filter {
            if !r.contains(&table_i) {
                info!("Skipping table number {} as it's not within the search range", table_i);

                continue;
            }
        }

        // Detect tables by their TH element count
        let req_len = match &collect {
            Include::All{ th_count, ..} => th_count,
            Include::Some{ th_count, ..} => th_count,
        };

        // Study table head to see if column count is what we're after
        if table_el.select(&th_sel).count().ne(req_len) {
            warn!("Skipping table number {} as it has {} columns, but {} are required", table_i, table_el.select(&th_sel).count(), req_len);
            continue;
        }

        // Rows in table, collect the ones with appropriate number of TD elements
        let table_tr_els = match &collect {
            Include::All{ td_map, ..} => table_el.select(&tr_sel)
                .filter(|e|e.select(&td_sel).count() == td_map.len())
                .collect::<Vec<ElementRef<'_>>>(),
            Include::Some{ td_map, ..} => {
                let rl = td_map.iter().filter(|(_, v)|v.is_some()).count();

                table_el.select(&tr_sel)
                    .filter(|e|e.select(&td_sel).count() >= rl)
                    .collect::<Vec<ElementRef<'_>>>()
                }
        };

        // Loop rows applying the provided selector to each cell or ignoring excluded columns
        for table_row in table_tr_els {
            let mut scraped = HashMap::new();
            let table_row_td_els = table_row.select(&td_sel).collect::<Vec<ElementRef<'_>>>();

            for (td_index, td_el) in table_row_td_els.into_iter().enumerate() {
                // See if this element is to be collected, and if so what's the rule
                let rule = match &collect {
                    Include::All{ td_map, ..} => match td_map.get(&td_index) {
                        Some(s) => s,
                        None => panic!("Stupid developer error: All columns set to be collected but selector for {} is undefined", td_index),
                    },
                    Include::Some{ td_map, ..} => match td_map.get(&td_index) {
                        Some(o) => match o {
                            Some(s) => s,
                            None => continue,
                        },
                        None => {
                            debug!("Limited columns set to be collected but selector for {} is undefined (Ignored column)", td_index);
                            continue;
                        },
                    },
                };

                match rule {
                    Select::Matching(s) => {
                        let sel = Selector::parse(s).map_err(|e|
                            anyhow!("Failed to parse selector from '{}': {}", s, e)
                        )?;

                        // Finds all elements matching the selector and stores them in the result map
                        let els = td_el.select(&sel).collect::<Vec<ElementRef<'_>>>();

                        scraped.insert(td_index, Found::Children(els));
                    },
                    Select::InnerAsText => {
                        let els = td_el.text()
                            .map(|t|t.to_string())
                            .collect::<Vec<String>>();

                        scraped.insert(td_index, Found::InnerText(els));
                    },
                    Select::TdElement => {
                        scraped.insert(td_index, Found::Parent(td_el));
                    }
                }

                
            }

            elements.push(scraped);
        }
    }
    
    return Ok(elements)
}

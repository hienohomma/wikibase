mod fetch;
mod map;
mod types;

use tokio::fs::{create_dir_all, read_to_string, write};
use std::{collections::BTreeMap, path::PathBuf, process::exit, vec};

use tracing::{debug, error, info, warn};
use types::{CallingCode, Capital, Currency, Flag, Identifier, Language, Region, SovereignState, UNMember};

const UN_NATIONS: &str = "https://www.un.org/en/about-us/member-states";
const SOVEREIGN_STATES: &str = "https://en.wikipedia.org/wiki/List_of_sovereign_states";
const FLAGS: &str = "https://en.wikipedia.org/wiki/Gallery_of_sovereign_state_flags";
const ISO_3166: &str = "https://en.wikipedia.org/wiki/List_of_ISO_3166_country_codes";
const CURRENCIES: &str = "https://en.wikipedia.org/wiki/List_of_circulating_currencies";
const EMOJIS: &str = "https://en.wikipedia.org/wiki/Regional_indicator_symbol";
const CALLING_CODES: &str = "https://en.wikipedia.org/wiki/List_of_country_calling_codes";
const LANG_CODES_ISO_639: &str = "https://en.wikipedia.org/wiki/List_of_ISO_639_language_codes";
const LANG_ZONES: &str = "https://en.wikipedia.org/wiki/List_of_official_languages_by_country_and_territory";
const CAPITALS: &str = "https://en.wikipedia.org/wiki/List_of_countries_and_dependencies_and_their_capitals_in_native_languages";


#[tokio::main]
async fn main() {
    // Boilerplate for tracing, set default loglevel to debug
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Read countries from the input file to have something to compare the findings with
    let input_countries = match read_to_string("input/countries.json").await {
        Ok(d) => serde_json::from_str::<BTreeMap<Identifier, Vec<String>>>(&d).unwrap(),
        Err(e) => {
            error!("Failed to read input countries data: {}", e);
            exit(1)
        }
    };

    // Collect all results to data directory for incremental building
    let dir = PathBuf::from("output");

    create_dir_all(&dir).await.unwrap();

    // Read and parse UN member states from un.org, country names are of interest
    let un_nations = match UNMember::fetch_un_nations(UN_NATIONS, &input_countries).await {
        Ok(n) => {
            info!("Fetched {} UN member states from {}", n.len(), UN_NATIONS);

            // Write UN member states to a file as json
            let json = serde_json::to_string_pretty(&n).unwrap();
            let un_nations_path = dir.join("un_nations.json");

            match write(&un_nations_path, json).await {
                Ok(_) => info!("UN member states data written to {}", un_nations_path.to_string_lossy()),
                Err(e) => {
                    error!("Failed to write UN member states data: {}", e);
                    exit(1)
                }
            }

            n
        },
        Err(e) => {
            error!("Failed to fetch UN member states data from {}: {}", UN_NATIONS, e);
            vec![]
        }
    };

    // Read and parse sovereign states from wikipedia, country names are of interest
    let mut sovereign_states_path = dir.to_owned();
    sovereign_states_path.push("sovereign_states.json");

    let mut countries = BTreeMap::new();

    if sovereign_states_path.exists() {
        match read_to_string(&sovereign_states_path).await {
            Ok(d) => countries = serde_json::from_str::<BTreeMap<Identifier, SovereignState>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read sovereign states data: {}", e);
                info!("Fetching sovereign states data again from {}", SOVEREIGN_STATES);
            }
        }
    }

    // Fetch sovereign states data if not read from file
    if countries.is_empty() {
        let html = match fetch::get_html(SOVEREIGN_STATES).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch sovereign states data: {}", e);
                exit(1)
            }
        
        };
    
        match SovereignState::from_html(&html, &un_nations, &input_countries) {
            Ok(n) => countries = n,
            Err(e) => {
                error!("Failed to parse sovereign states data: {}", e);
                exit(1)
            }
        };   
    }

    // Write sovereign states to a file as json
    let json = serde_json::to_string_pretty(&countries).unwrap();

    match write(&sovereign_states_path, json).await {
        Ok(_) => info!("Sovereign states data written to {}", sovereign_states_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write sovereign states data: {}", e);
            exit(1)
        }
    }

    // Check if we have 193? UN member states
    let un_nation_count = match un_nations.is_empty() {
        true => 193,
        false => un_nations.len()
    };

    let un_member_states = countries.iter().filter_map(|t|match t.1.un_member {
        true => Some((t.0.to_owned(), t.1.to_owned())),
        false => None
    }).collect::<BTreeMap<Identifier, SovereignState>>();
    
    if un_member_states.len() != un_nation_count {
        let not = countries.values().filter(|s|!s.un_member).collect::<Vec<&SovereignState>>();
        let nons = not.iter().map(|s|s.to_string()).collect::<Vec<String>>().join(", ");

        error!("Expected 193 UN member states, got {}", un_member_states.len());
        warn!("Current non members are: {}", nons);
    }

    // Exclude non UN member states (according to wikipedia)
    countries = countries.into_iter().filter(|(_, s)|s.un_member).collect();
    
    info!("Proceeding with {} UN member states", un_member_states.len());

    // Read and parse ISO 3166 codes from wikipedia, compare findings with our list of UN member states
    let mut regions = BTreeMap::new();
    let regions_path = dir.join("regions.json");

    if regions_path.exists() {
        match read_to_string(&regions_path).await {
            Ok(d) => regions = serde_json::from_str::<BTreeMap<Identifier, Region>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read ISO 3166 data: {}", e);
                info!("Fetching ISO 3166 data again from {}", ISO_3166);
            }
        }
    }

    if regions.is_empty() {
        let html = match fetch::get_html(ISO_3166).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch ISO 3166 data: {}", e);
                exit(1)
            }
        };
    
        regions = match Region::from_html(&html, &un_member_states) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to parse ISO 3166 data to regions: {}", e);
                exit(1)
            }
        };
    }

    // Write ISO 3166 codes to a file as json
    let json = serde_json::to_string_pretty(&regions).unwrap();

    match write(&regions_path, json).await {
        Ok(_) => info!("ISO 3166 regions data written to {}", regions_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write ISO 3166 data: {}", e);
            exit(1)
        }
    }

    // Check if we have flags for found countries
    let mut flags = BTreeMap::new();
    let mut flags_missing = BTreeMap::new();
    let mut flags_dir = dir.to_owned();
    flags_dir.push("flags");

    for (id, country) in countries.iter() {
        // Only take flags of countries we're after
        if !un_member_states.contains_key(id) {
            continue;
        }

        let mut path = flags_dir.clone();
        path.push(id.as_str());
        path.push("source.png");

        // Expected format if flag has been added already
        if path.exists() {
            path.pop();

            debug!("Flag for {} found at {}", country.name_short, path.to_string_lossy());
            
            flags.insert(id.to_owned(), Flag::new(id.to_owned(), path));
            continue;
        }

        flags_missing.insert(id.to_owned(), country.to_owned());
    }

    // Try to download missing flags
    if flags_missing.len() > 0 {
        info!("Found {} missing flags, trying to fetch...", flags_missing.len());

        let html = match fetch::get_html(FLAGS).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch flags data: {}", e);
                exit(1)
            }
        };

        info!("Fetched flags data from {}", FLAGS);

        // Try to fetch flags
        match Flag::from_html(&html, &flags_missing, &flags_dir).await {
            Ok(f) =>  {
                info!("Downloaded {} flags", f.len());
                
                for i in f {
                    flags.insert(i.sovereignity.to_owned(), i);
                }
            },
            Err(e) => {
                error!("Failed to download flag: {}", e);
                exit(1)
            }
        }
    }

    // Write flags data to a file as json
    let flags_path = dir.join("flags.json");
    let json = serde_json::to_string_pretty(&flags).unwrap();

    match write(&flags_path, json).await {
        Ok(_) => info!("Flags data written to {}", flags_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write flags data: {}", e);
            exit(1)
        }
    }

    // Run transformations on the flags if not present
    if let Err(e )= Flag::transform_flags(&flags).await {
        error!("Failed to transform flags: {}", e);
        exit(1)
    }
    
    // Read and parse currencies from wikipedia, compare findings with our list of UN member states
    let mut currencies = BTreeMap::new();
    let currencies_path = dir.join("currencies.json");

    if currencies_path.exists() {
        match read_to_string(&currencies_path).await {
            Ok(d) => currencies = serde_json::from_str::<BTreeMap<Identifier, Currency>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read currencies data: {}", e);
                info!("Fetching currencies data again from {}", CURRENCIES);
            }
        }
    }

    if currencies.is_empty() {
        let html = match fetch::get_html(CURRENCIES).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch currencies data: {}", e);
                exit(1)
            }
        };
    
        currencies = match Currency::from_html(&html, &regions, Some(&input_countries)) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to parse currencies data: {}", e);
                exit(1)
            }
        };
    }

    // Write currencies to a file as json
    let json = serde_json::to_string_pretty(&currencies).unwrap();

    match write(&currencies_path, json).await {
        Ok(_) => info!("Currencies data written to {}", currencies_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write currencies data: {}", e);
            exit(1)
        }
    }

    // Read and parse flag emojis from wikipedia, then extend our flags data with emojis
    let emoji_path = dir.join("emojis.json");
    let mut emojis = BTreeMap::new();

    if emoji_path.exists() {
        match read_to_string(&emoji_path).await {
            Ok(d) => emojis = serde_json::from_str::<BTreeMap<Identifier, String>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read emojis data: {}", e);
                info!("Fetching emojis data again from {}", EMOJIS);
            }
        }
    }

    // Only proceed if we don't have emojis
    if emojis.is_empty() {
        let html = match fetch::get_html(EMOJIS).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch emojis data: {}", e);
                exit(1)
            }
        };
    
        match Flag::emojis_from_html(&html, &regions) {
            Ok(n) => emojis = n,
            Err(e) => {
                error!("Failed to parse emojis data: {}", e);
                exit(1)
            }
        };
    }

    // Write flags json again with emojis
    let json = serde_json::to_string_pretty(&emojis).unwrap();

    match write(&emoji_path, json).await {
        Ok(_) => info!("Emoji flag data written to {}", emoji_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write emoji flag data: {}", e);
            exit(1)
        }
    }

    // Read and parse calling codes from wikipedia, take the ones we have in our list of UN member states
    let mut calling_codes = BTreeMap::new();
    let calling_codes_path = dir.join("calling_codes.json");

    if calling_codes_path.exists() {
        match read_to_string(&calling_codes_path).await {
            Ok(d) => calling_codes = serde_json::from_str::<BTreeMap<Identifier, CallingCode>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read calling codes data: {}", e);
                info!("Fetching calling codes data again from {}", CALLING_CODES);
            }
        }
    }

    if calling_codes.is_empty() {
        let html = match fetch::get_html(CALLING_CODES).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch calling codes data: {}", e);
                exit(1)
            }
        };
    
        calling_codes = match CallingCode::from_html(&html, &regions, Some(&input_countries)) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to parse calling codes data: {}", e);
                exit(1)
            }
        };
    }

    // Write calling codes to a file as json
    let json = serde_json::to_string_pretty(&calling_codes).unwrap();

    match write(&calling_codes_path, json).await {
        Ok(_) => info!("Calling codes data written to {}", calling_codes_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write calling codes data: {}", e);
            exit(1)
        }
    }

    // Read and parse ISO 639 language codes from wikipedia, compare findings with our list of UN member states
    let mut languages = BTreeMap::new();
    let languages_path = dir.join("languages.json");

    if languages_path.exists() {
        match read_to_string(&languages_path).await {
            Ok(d) => languages = serde_json::from_str::<BTreeMap<Identifier, Language>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read languages data: {}", e);
                info!("Fetching languages data again from {}", LANG_CODES_ISO_639);
            }
        }
    }

    if languages.is_empty() {
        let html = match fetch::get_html(LANG_CODES_ISO_639).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch languages data: {}", e);
                exit(1)
            }
        };
    
        languages = match Language::from_html(&html) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to parse languages data: {}", e);
                exit(1)
            }
        };
    }

    // Read and parse languages spoken in different regions and match regions with existing languages
    match fetch::get_html(LANG_ZONES).await {
        Ok(d) => if let Err(e) = Language::zones_from_html(&d, &input_countries, &regions, &mut languages) {
            error!("Failed to parse languages zones data: {}", e);
        },
        Err(e) => {
            error!("Failed to fetch languages data: {}", e);
        }
    }

    // Write languages to a file as json
    let json = serde_json::to_string_pretty(&languages).unwrap();

    match write(&languages_path, json).await {
        Ok(_) => info!("Languages data written to {}", languages_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write languages data: {}", e);
            exit(1)
        }
    }

    // Read and parse capitals from wikipedia. Take capitals of regions present in out list
    let mut capitals = BTreeMap::new();
    let capitals_path = dir.join("capitals.json");

    if capitals_path.exists() {
        match read_to_string(&capitals_path).await {
            Ok(d) => capitals = serde_json::from_str::<BTreeMap<Identifier, Capital>>(&d).unwrap(),
            Err(e) => {
                error!("Failed to read capitals data: {}", e);
                info!("Fetching capitals data again from {}", CAPITALS);
            }
        }
    }

    if capitals.is_empty() {
        let html = match fetch::get_html(CAPITALS).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to fetch capitals data: {}", e);
                exit(1)
            }
        };
    
        capitals = match Capital::from_html(&html, &regions, Some(&input_countries)) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to parse capitals data: {}", e);
                exit(1)
            }
        };
    }

    // Write capitals to a file as json
    let json = serde_json::to_string_pretty(&capitals).unwrap();

    match write(&capitals_path, json).await {
        Ok(_) => info!("Capitals data written to {}", capitals_path.to_string_lossy()),
        Err(e) => {
            error!("Failed to write capitals data: {}", e);
            exit(1)
        }
    }

    info!("All data collected and written to output directory");
}

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fmt::Result as Formatted;
use std::path::PathBuf;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use image::{ImageBuffer, Rgba, RgbaImage};
use image::{io::Reader as ImageReader, DynamicImage};
use imageproc::drawing::draw_filled_circle_mut;
use tokio::fs::{create_dir_all, write};
use tokio::task::JoinSet;
use anyhow::{anyhow, bail, Result};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::map::{map_from_table_data, Found, Include, Select};
use super::{link_title_if, Identifier, Region, SovereignState};


const TRANSPARENT: Rgba<u8> = image::Rgba::<u8>([0, 0, 0, 0]);
const WHITE: Rgba<u8> = image::Rgba::<u8>([255, 255, 255, 255]);
const BLACK: Rgba<u8> = image::Rgba::<u8>([0, 0, 0, 255]);
const BLUE: Rgba<u8> = image::Rgba::<u8>([0, 0, 255, 255]);
const GREEN: Rgba<u8> = image::Rgba::<u8>([0, 255, 0, 255]);
const RED: Rgba<u8> = image::Rgba::<u8>([255, 0, 0, 255]);
const YELLOW: Rgba<u8> = image::Rgba::<u8>([255, 255, 0, 255]);

const TRANSFORMATIONS: [&str; 7] = [
    "round.png", "round_bl.png", "round_wh.png", "round_b.png", "round_g.png", "round_y.png", "round_r.png"
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    pub sovereignity: Identifier,
    pub dir: PathBuf,
}

impl Display for Flag {
    fn fmt(&self, f: &mut Formatter) -> Formatted {
        write!(f, "{} flag", self.sovereignity)
    }
}

impl Flag {
    pub fn new(sovereignity: Identifier, dir: PathBuf) -> Self {
        Self {
            sovereignity,
            dir
        }
    }
    pub async fn from_html(html: &Html, countries: &BTreeMap<Identifier, SovereignState>, dir: &PathBuf) -> Result<Vec<Self>> {
        let mut flags = vec![];
        let mut handles = JoinSet::new();
        
        for(id, country) in countries {
            let mut dir = dir.clone();
            dir.push(id.as_str());
    
            // Primarly try to match the short name of the country
            let selector = Selector::parse(&format!("img[alt=\"{}\"]", country.name_short)).unwrap();
            let mut urls = html.select(&selector).filter_map(|i|i.value().attr("src")).collect::<Vec<&str>>();
    
            // Secondary try to match the long name of the country
            if urls.len() == 0 {
                let selector = Selector::parse(&format!("img[alt=\"{}\"]", country.name_long)).unwrap();
                urls = html.select(&selector).filter_map(|i|i.value().attr("src")).collect();
            }
    
            // Then as a last resort try with short name as the start of the alt attribute
            if urls.len() == 0 {
                let selector = Selector::parse(&format!("img[alt^=\"{}\"]", country.name_short)).unwrap();
                urls = html.select(&selector).filter_map(|i|i.value().attr("src")).collect();
            }
    
            let url = match urls.len() {
                0 => bail!("Flag for {} not found", country.name_short),
                i => {
                    if i > 1 {
                        warn!("Found multiple flag urls for {}, using the first one", country.name_short);
                    }
    
                    urls.into_iter().next().ok_or(anyhow!("Failed to get flag url for {}", country.name_short))?
                }
            };
    
            // Parallel fetching and processing of flags
            let flag_dir = dir.clone();
            let url = url.to_owned();
            let iso_id = id.clone();
    
            handles.spawn(async move {
                let attempts = 5;
    
                for i in 1..attempts {
                    if try_flag_download(&url, &flag_dir).await.is_ok() {
                        return Ok(Self::new(iso_id, flag_dir));
                    }

                    // Wait for a while before retrying
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    
                    match i == attempts {
                        true => warn!("All {} attempts to download {} flag failed", iso_id, attempts),
                        false => warn!("Failed to download {} flag. Attempt {}/{} Retrying...", iso_id, i, attempts),
                    }
                }
    
                bail!("Failed to download {} flag after {} attempts", iso_id, attempts)
            });
        }

        while let Some(r) = handles.join_next().await {
            r?.map(|f|flags.push(f))?;
        }

        Ok(flags)
    }
    pub fn emojis_from_html(html: &Html, regions: &BTreeMap<Identifier, Region>) -> Result<BTreeMap<Identifier, String>> {
        let mut cols = HashMap::new();
        cols.insert(0, Some(Select::Matching("a")));
        cols.insert(1, Some(Select::InnerAsText));
        cols.insert(2, None);
        cols.insert(3, None);

        let mut items = BTreeMap::new();

        for m in map_from_table_data(html, Include::Some { th_count: 4, td_map: cols }, None)? {
            let emoji = match m.get(&0).unwrap() {
                Found::Children(c) => c.into_iter()
                    .find_map(|e|link_title_if("/wiki/", *e))
                    .ok_or(anyhow!("Failed to read country flag emoji"))?,
                _ => bail!("Expected TD elements for flag emoji")
            };

            // Read 2 letter country code (iso 3166) from the table
            let iso_id = match m.get(&1).unwrap() {
                Found::InnerText(v) => match v.into_iter().find(|s|s.trim().len() == 2) {
                    Some(s) => Identifier::new(s),
                    None => bail!("Failed to read country iso code from flag table"),
                },
                _ => bail!("Expected inner text for country iso code"),
            };

            // Find the region from flags by iso 3166 identifier
            if regions.get(&iso_id).is_none() {
                warn!("Skipping emoji {}: Region iso code {} not found", emoji, iso_id);
                continue;
            };

            // Check if we already have this emoji in the list
            if items.contains_key(&iso_id) {
                warn!("Skipping emoji for {}: Duplicate entry", iso_id);
                continue;
            }

            items.insert(iso_id, emoji);
        }

        Ok(items)
    }
    pub async fn transform_flags(flags: &BTreeMap<Identifier, Self>) -> Result<()> {
        for (i, f) in flags {
            let mut path = f.dir.clone();
            path.push("source.png");

            if !path.is_file() {
                bail!("Flag source image for {} not found from {:?}", i, path)
            }

            let missing = TRANSFORMATIONS.iter()
                .filter_map(|t|{
                    let p = f.dir.join(t);
                    
                    match p.is_file() {
                        true => None,
                        false => Some((*t, p))
                    }
                })
                .collect::<Vec<(&str, PathBuf)>>();

            if missing.is_empty() {
                debug!("All transformations for {} already exist", i);
                continue;
            }

            let mut img = image_reader(&path)?;

            // Crop image into a max sized square for transformations that expect a square image
            let (mut side, x, y) = match img.width() >= img.height() {
                true => (
                    img.height(),
                    (img.width() - img.height()) / 2,
                    0
                ),
                false => (
                    img.width(),
                    0,
                    (img.height() - img.width()) / 2,
                )
            };

            // Make sure our square sides are odd numbers
            if side % 2 == 0 {
                side -= 1;
            }

            // Crop image into a square
            img = img.crop_imm(x, y, side, side);
            let side = side as i32;

            info!("Transforming flag {} into {} variations", i, missing.len());

            for (t, p) in missing {
                match t {
                    "round.png" => {
                        let round = round_from_rect(&img.to_rgba8(), side);
                        png_writer(&round, &p).await?;
                    },
                    "round_bl.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, WHITE, BLACK);
                        png_writer(&round, &p).await?;
                    },
                    "round_wh.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, BLACK, WHITE);
                        png_writer(&round, &p).await?;
                    },
                    "round_b.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, WHITE, BLUE);
                        png_writer(&round, &p).await?;
                    },
                    "round_r.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, WHITE, RED);
                        png_writer(&round, &p).await?;
                    },
                    "round_g.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, WHITE, GREEN);
                        png_writer(&round, &p).await?;
                    },
                    "round_y.png" => {
                        let round = framed_round_from_rect(&img.to_rgba8(), side, WHITE, YELLOW);
                        png_writer(&round, &p).await?;
                    },
                    _ => unreachable!()
                }
            }
        }

        Ok(())
    }
}

fn image_reader(path: &PathBuf) -> Result<DynamicImage> {
    // Read source image from file
    let reader = ImageReader::open(path)?
        .with_guessed_format()?;

    match reader.format() {
        Some(f) => {
            if f != image::ImageFormat::Png {
                bail!("Expected flag image to be in PNG format")
            }
        },
        None => bail!("Unable to detect image format from flag file."),
    }

    // Image is valid PNG image
    reader.decode().map_err(|e|anyhow!("Failed to decode flag image: {}", e))
}

fn round_from_rect(buf: &ImageBuffer<Rgba<u8>, Vec<u8>>, size: i32) -> DynamicImage {
    // Draw a white circle on a transparent background that is the same size as the cropped image
    let half = size / 2;
    let mut img = RgbaImage::from_pixel(size as u32, size as u32, TRANSPARENT);
    
    draw_filled_circle_mut(
        &mut img,
        (half, half),
        half,
        WHITE
    );

    substitute_color_px(&mut img, buf, WHITE);
    
    DynamicImage::ImageRgba8(img)
}

fn framed_round_from_rect(buf: &ImageBuffer<Rgba<u8>, Vec<u8>>, size: i32, substitute: Rgba<u8>, frame: Rgba<u8>) -> DynamicImage {
    // Draw a white circle on a transparent background that is the same size as the cropped image
    let half = size / 2;
    let mut img = RgbaImage::from_pixel(size as u32, size as u32, TRANSPARENT);

    // Draw a frame around the circle
    draw_filled_circle_mut(
        &mut img,
        (half, half),
        half - 2,
        frame
    );
    
    // Draw a (placeholder color) circle inside the frame
    draw_filled_circle_mut(
        &mut img,
        (half, half),
        half - 4,
        substitute
    );

    // Substitute the placeholder color with the flag image
    substitute_color_px(&mut img, buf, substitute);
    
    // Blur the image to make the frame look better
    let mut img = DynamicImage::ImageRgba8(img)
        .blur(0.6)
        .into_rgba8();

    // Draw new smaller (placeholder color) circle inside blurred image
    draw_filled_circle_mut(
        &mut img,
        (half, half),
        half - 6,
        substitute
    );

    // Substitute the placeholder color with the flag image
    substitute_color_px(&mut img, buf, substitute);

    DynamicImage::ImageRgba8(img)
}

fn substitute_color_px(target: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, source: &ImageBuffer<Rgba<u8>, Vec<u8>>, color: Rgba<u8>) {
    for (x, y, p) in target.enumerate_pixels_mut() {
        if color.eq(*&p) {
            p.0 = source.get_pixel(x, y).0;
        }
    }
}

async fn png_writer(image: &DynamicImage, path: &PathBuf) -> Result<()> {
    let mut buf = vec![];

    PngEncoder::new(&mut buf).write_image(
        image.as_bytes(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    ).map_err(|e| anyhow!("Failed to create PNG image: {}", e))?;

    write(path, buf).await.map_err(|e|
        anyhow!("Failed to write transformed flag as PNG to {}: {}", path.to_string_lossy(), e)
    )
}

async fn try_flag_download(url: &String, flag_dir: &PathBuf) -> Result<DynamicImage> {
    let bytes = match url.starts_with("//") {
        true => crate::fetch::get_bytes(format!("https:{}", url)).await,
        false => crate::fetch::get_bytes(url).await,
    }?;

    if bytes.is_empty() {
        bail!("Unable to read bytes from {}", url);
    }

    if let Err(e) = create_dir_all(flag_dir).await {
        bail!("Failed to create flags directory: {}", e)
    }

    let extension = url.split(".").last();
    let mut file = flag_dir.clone();
    file.push("source");
    file.set_extension(extension.unwrap_or("png"));

    if file.extension() != Some(OsStr::new("png")) {
        bail!("Expected flag file to be in png format")
    }

    write(&file, &bytes)
        .await
        .map_err(|e|
            anyhow!("Failed to write flag file to {}: {}", file.to_string_lossy(), e)
        )?;
    
    match image_reader(&file) {
        Ok(i) => Ok(i),
        Err(e) => {
            tokio::fs::remove_file(&file).await?;
            Err(e)
        }
    }
}
use scraper::Html;
use anyhow::{Result, anyhow};


pub async fn get_html(url: &str) -> Result<Html> {
    let resp = reqwest::get(url).await.map_err(|e| anyhow!("Failed to open http document from: {}", e))?;
    let html = resp.text().await.map_err(|e| anyhow!("Failed to open http document from: {}", e))?;

    // Use scraper to build readable html from response data
    Ok(Html::parse_document(&html))
}

pub async fn get_bytes<T>(url: T) -> Result<Vec<u8>> where T: AsRef<str> {
    let resp = reqwest::get(url.as_ref()).await.map_err(|e| anyhow!("Failed to open http document from: {}", e))?;
    let bytes = resp.bytes().await.map_err(|e| anyhow!("Failed to open http document from: {}", e))?;

    Ok(bytes.to_vec())
}

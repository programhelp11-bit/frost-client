//! The launcher's local cosmetics library: skins and capes the player drew or saved, plus
//! favourite community capes. The in-game radial menus (R = skins, X = capes) show these.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::data_dir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// `skin` | `cape` (own drawing, local) | `fav` (favourite community cape, `ref_id` = server id)
    pub kind: String,
    #[serde(default)]
    pub slim: bool,
    #[serde(default)]
    pub ref_id: Option<String>,
    /// Average colour `RRGGBB`, used as a swatch in the radial menu.
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub created: u64,
}

fn dir() -> PathBuf {
    data_dir().join("library")
}

fn index_path() -> PathBuf {
    dir().join("index.json")
}

pub fn list() -> Vec<Item> {
    std::fs::read(index_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_index(items: &[Item]) -> Result<()> {
    std::fs::create_dir_all(dir())?;
    std::fs::write(index_path(), serde_json::to_vec_pretty(items)?).context("write library index")
}

fn new_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Average colour of the opaque pixels in a region of the PNG (whole image when `region` is None).
pub fn average_color(png: &[u8], region: Option<(u32, u32, u32, u32)>) -> String {
    let Ok(img) = image::load_from_memory(png) else { return "5A8CFF".into() };
    let rgba = img.to_rgba8();
    let (rx, ry, rw, rh) = region.unwrap_or((0, 0, rgba.width(), rgba.height()));
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for y in ry..(ry + rh).min(rgba.height()) {
        for x in rx..(rx + rw).min(rgba.width()) {
            let p = rgba.get_pixel(x, y);
            if p[3] > 40 {
                r += u64::from(p[0]);
                g += u64::from(p[1]);
                b += u64::from(p[2]);
                n += 1;
            }
        }
    }
    if n == 0 {
        return "5A8CFF".into();
    }
    format!("{:02X}{:02X}{:02X}", r / n, g / n, b / n)
}

/// Save a skin (`kind` = "skin") or a drawn cape (`kind` = "cape").
pub fn save(kind: &str, name: &str, png: &[u8], slim: bool) -> Result<Item> {
    let (kind, region) = match kind {
        "skin" => {
            crate::cosmetics::validate_skin_png(png)?;
            ("skin", Some((8, 8, 8, 8)))
        }
        "cape" => {
            crate::cosmetics::validate_cape_png(png)?;
            ("cape", Some((1, 1, 10, 16)))
        }
        other => bail!("unknown library kind {other}"),
    };
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 32 {
        bail!("name must be 1-32 characters");
    }
    let id = new_id();
    std::fs::create_dir_all(dir())?;
    std::fs::write(dir().join(format!("{id}.png")), png)?;
    let item = Item {
        id,
        name: name.to_owned(),
        kind: kind.to_owned(),
        slim,
        ref_id: None,
        color: average_color(png, region),
        created: crate::config::unix_now(),
    };
    let mut items = list();
    items.push(item.clone());
    write_index(&items)?;
    Ok(item)
}

/// Remember a community cape so it shows up in the in-game cape wheel.
pub fn add_favorite(cape_id: &str, name: &str, png: &[u8]) -> Result<Item> {
    let mut items = list();
    if let Some(existing) = items.iter().find(|i| i.kind == "fav" && i.ref_id.as_deref() == Some(cape_id)) {
        return Ok(existing.clone());
    }
    let item = Item {
        id: new_id(),
        name: name.to_owned(),
        kind: "fav".into(),
        slim: false,
        ref_id: Some(cape_id.to_owned()),
        color: average_color(png, Some((1, 1, 10, 16))),
        created: crate::config::unix_now(),
    };
    items.push(item.clone());
    write_index(&items)?;
    Ok(item)
}

pub fn load_png(id: &str) -> Result<Vec<u8>> {
    if !safe_id(id) {
        bail!("bad id");
    }
    std::fs::read(dir().join(format!("{id}.png"))).map_err(|_| anyhow!("that item has no image"))
}

pub fn get(id: &str) -> Option<Item> {
    list().into_iter().find(|i| i.id == id)
}

pub fn remove(id: &str) -> Result<()> {
    if !safe_id(id) {
        bail!("bad id");
    }
    let mut items = list();
    items.retain(|i| i.id != id);
    write_index(&items)?;
    let _ = std::fs::remove_file(dir().join(format!("{id}.png")));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn average_color_of_solid_image() {
        assert_eq!(average_color(&solid(64, 32, [255, 0, 128, 255]), None), "FF0080");
        assert_eq!(average_color(&solid(64, 32, [255, 0, 128, 0]), None), "5A8CFF"); // fully transparent
        assert_eq!(average_color(b"junk", None), "5A8CFF");
    }

    #[test]
    fn ids_are_validated() {
        assert!(safe_id("abc123"));
        assert!(!safe_id("../etc"));
        assert!(!safe_id(""));
    }
}

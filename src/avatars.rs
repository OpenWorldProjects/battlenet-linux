//! Public portrait images only; no account credentials are sent to the CDN.
use eframe::egui::{self, ColorImage, TextureHandle};
use std::{
    collections::HashMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Cache {
    entries: HashMap<u32, Entry>,
    worker: Option<mpsc::SyncSender<(u32, egui::Context)>>,
    results: Option<mpsc::Receiver<(u32, Option<ColorImage>)>>,
}
struct Entry {
    texture: Option<TextureHandle>,
    requested: Instant,
}
impl Cache {
    pub fn texture(&mut self, ctx: &egui::Context, id: u32) -> Option<TextureHandle> {
        let id = if id == 0 { 1 } else { id };
        if id > 1_000_000 {
            return None;
        }
        if self.worker.is_none() {
            let (tx, rx) = mpsc::sync_channel::<(u32, egui::Context)>(256);
            let (done, results) = mpsc::channel();
            self.worker = Some(tx);
            self.results = Some(results);
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .redirect(reqwest::redirect::Policy::none())
                    .build();
                for (id, ctx) in rx {
                    let image = client.as_ref().ok().and_then(|client| download(client, id));
                    if done.send((id, image)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
        for (id, image) in self.results.as_ref().unwrap().try_iter() {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.texture = image.map(|image| {
                    ctx.load_texture(format!("avatar-{id}"), image, egui::TextureOptions::LINEAR)
                });
            }
        }
        let retry = self.entries.get(&id).is_none_or(|entry| {
            entry.texture.is_none() && entry.requested.elapsed() > Duration::from_secs(300)
        });
        if retry {
            // Bound GPU memory even with an unusually large friend list.
            if self.entries.len() >= 512 && !self.entries.contains_key(&id) {
                return None;
            }
            if self
                .worker
                .as_ref()
                .unwrap()
                .try_send((id, ctx.clone()))
                .is_ok()
            {
                self.entries.insert(
                    id,
                    Entry {
                        texture: None,
                        requested: Instant::now(),
                    },
                );
            }
        }
        self.entries.get(&id).and_then(|e| e.texture.clone())
    }
}
fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(8 * 1024 * 1024);
    reader.limits(limits);
    let mut image = reader
        .decode()
        .ok()?
        .resize_exact(80, 80, image::imageops::FilterType::Triangle)
        .to_rgba8();
    // Antialiased circular portrait, shared by header and friend rows.
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let distance = ((x as f32 + 0.5 - 40.0).powi(2) + (y as f32 + 0.5 - 40.0).powi(2)).sqrt();
        pixel[3] = (pixel[3] as f32 * (40.0 - distance).clamp(0.0, 1.0)) as u8;
    }
    Some(ColorImage::from_rgba_unmultiplied([80, 80], &image))
}
fn download(client: &reqwest::blocking::Client, id: u32) -> Option<ColorImage> {
    let directory = directories::ProjectDirs::from("", "", "battlenet-linux-launcher")?
        .cache_dir()
        .join("avatars");
    let path = directory.join(format!("{id}.jpg"));
    if let Ok(metadata) = path.metadata() {
        if metadata.len() <= 512 * 1024
            && metadata.modified().ok()?.elapsed().ok()? < Duration::from_secs(7 * 86400)
        {
            if let Some(image) = std::fs::read(&path).ok().and_then(|b| decode(&b)) {
                return Some(image);
            }
        }
    }
    let response = client
        .get(format!(
            "https://blzprofile.akamaized.net/static/avatar/account/{id}.jpg"
        ))
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let mut bytes = Vec::new();
    response.take(512 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 512 * 1024 {
        return None;
    }
    let image = decode(&bytes)?;
    if std::fs::create_dir_all(&directory).is_ok() {
        let temporary = directory.join(format!("{id}-{}.tmp", std::process::id()));
        if std::fs::write(&temporary, &bytes).is_ok() {
            let _ = std::fs::rename(&temporary, &path);
        }
    }
    Some(image)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_images_are_rejected_and_portraits_are_circular() {
        assert!(decode(b"not an image").is_none());
        let source = image::RgbaImage::from_pixel(16, 16, image::Rgba([100, 120, 140, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        source
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = decode(bytes.get_ref()).unwrap();
        assert_eq!(image.size, [80, 80]);
        assert_eq!(image.pixels[0].a(), 0);
        assert!(image.pixels[40 * 80 + 40].a() > 0);
    }
}

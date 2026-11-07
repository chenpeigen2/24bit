//! Cover art loading.
//!
//! Search results carry cover URLs, and the site's two catalogues use different
//! image hosts. Images are fetched on a background thread and decoded once, then
//! kept as textures — searching repeatedly must not re-download or re-decode.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

/// What the UI needs to know about one cover.
#[derive(Clone)]
pub enum CoverState {
    /// Not requested yet.
    Pending,
    /// Decoded and ready.
    Ready(egui::TextureHandle),
    /// The URL could not be loaded.
    Failed,
}

/// A URL-keyed cover store with a background fetcher.
pub struct Covers {
    requested: HashMap<String, CoverState>,
    to_worker: Sender<String>,
    from_worker: Receiver<(String, Option<ColorImage>)>,
}

use egui::ColorImage;

impl Covers {
    /// Start the fetcher thread.
    pub fn new(ctx: egui::Context) -> Self {
        let (to_worker, req_rx) = std::sync::mpsc::channel::<String>();
        let (res_tx, from_worker) = std::sync::mpsc::channel::<(String, Option<ColorImage>)>();

        std::thread::Builder::new()
            .name("24bit-covers".into())
            .spawn(move || {
                let client = match reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => return,
                };

                while let Ok(url) = req_rx.recv() {
                    let image = fetch_and_decode(&client, &url);
                    if res_tx.send((url, image)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .ok();

        Self {
            requested: HashMap::new(),
            to_worker,
            from_worker,
        }
    }

    /// Ask for a cover. Idempotent; repeated calls do nothing.
    pub fn request(&mut self, url: &str) {
        if url.is_empty() || self.requested.contains_key(url) {
            return;
        }
        self.requested
            .insert(url.to_owned(), CoverState::Pending);
        let _ = self.to_worker.send(url.to_owned());
    }

    /// Apply finished downloads to the texture store.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((url, maybe_image)) = self.from_worker.try_recv() {
            match maybe_image {
                Some(image) => {
                    let handle = ctx.load_texture(
                        format!("cover:{url}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.requested.insert(url, CoverState::Ready(handle));
                }
                None => {
                    self.requested.insert(url, CoverState::Failed);
                }
            }
        }
    }

    /// Look up a cover's state.
    pub fn get(&self, url: &str) -> Option<&CoverState> {
        self.requested.get(url)
    }
}

fn fetch_and_decode(client: &reqwest::blocking::Client, url: &str) -> Option<ColorImage> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

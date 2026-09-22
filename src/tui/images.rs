//! Inline images: download in the background, encode once per size, draw.
//!
//! A URL is fetched the first time a frame asks for it. Until it arrives the
//! area shows a placeholder, so the layout never jumps. Encoded protocols are
//! cached per (URL, size) because encoding (and, for kitty, transmitting) is
//! the expensive part.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use image::DynamicImage;
use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Image, Resize};

/// Largest image body bs downloads.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Parallel downloads.
const LOADERS: usize = 4;

enum Slot {
    Loading,
    Ready(DynamicImage),
    Failed,
}

type Loaded = (String, Result<DynamicImage, String>);

/// Cache and renderer for every image on screen.
pub struct Images {
    picker: Picker,
    slots: HashMap<String, Slot>,
    protocols: HashMap<(String, u16, u16), Protocol>,
    tx: Sender<String>,
    rx: Receiver<Loaded>,
}

impl Images {
    /// Start the loader threads.
    pub fn new(picker: Picker) -> Self {
        let (url_tx, url_rx) = channel::<String>();
        let (img_tx, img_rx) = channel::<Loaded>();
        let url_rx = Arc::new(Mutex::new(url_rx));
        for _ in 0..LOADERS {
            let url_rx = Arc::clone(&url_rx);
            let img_tx = img_tx.clone();
            thread::spawn(move || {
                let agent = crate::api::agent();
                loop {
                    let url = match url_rx.lock().expect("loader queue").recv() {
                        Ok(url) => url,
                        Err(_) => return,
                    };
                    let result = fetch(&agent, &url);
                    if img_tx.send((url, result)).is_err() {
                        return;
                    }
                }
            });
        }
        Self {
            picker,
            slots: HashMap::new(),
            protocols: HashMap::new(),
            tx: url_tx,
            rx: img_rx,
        }
    }

    /// The protocol images are drawn with.
    pub fn protocol_type(&self) -> ratatui_image::picker::ProtocolType {
        self.picker.protocol_type()
    }

    /// Pixel size of one terminal cell, as the terminal reported it.
    pub fn cell_size(&self) -> (u16, u16) {
        let f = self.picker.font_size();
        (f.width, f.height)
    }

    /// Collect finished downloads. Returns whether any arrived.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((url, result)) = self.rx.try_recv() {
            let slot = match result {
                Ok(img) => Slot::Ready(img),
                Err(_) => Slot::Failed,
            };
            self.slots.insert(url, slot);
            changed = true;
        }
        changed
    }

    /// Whether any requested image is still downloading.
    pub fn loading(&self) -> bool {
        self.slots.values().any(|s| matches!(s, Slot::Loading))
    }

    /// Draw `url` fitted inside `area`, starting its download when needed.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, url: &str) {
        let area = area.intersection(frame.area());
        if area.width == 0 || area.height == 0 || url.is_empty() {
            return;
        }
        let slot = self.slots.entry(url.to_string()).or_insert_with(|| {
            let _ = self.tx.send(url.to_string());
            Slot::Loading
        });
        let img = match slot {
            Slot::Ready(img) => img,
            Slot::Loading => return placeholder(frame, area, "…"),
            Slot::Failed => return placeholder(frame, area, "×"),
        };
        let key = (url.to_string(), area.width, area.height);
        if !self.protocols.contains_key(&key) {
            let size = Size::new(area.width, area.height);
            // Scale rather than Fit: a thumbnail smaller than its box is
            // enlarged to fill it, keeping its proportions.
            let resize = Resize::Scale(Some(FilterType::Triangle));
            match self.picker.new_protocol(img.clone(), size, resize) {
                Ok(p) => {
                    self.protocols.insert(key.clone(), p);
                }
                Err(_) => return placeholder(frame, area, "×"),
            }
        }
        frame.render_widget(Image::new(&self.protocols[&key]), area);
    }
}

fn placeholder(frame: &mut Frame, area: Rect, mark: &str) {
    frame.render_widget(Paragraph::new(mark).style(Style::new().dark_gray()), area);
}

fn fetch(agent: &ureq::Agent, url: &str) -> Result<DynamicImage, String> {
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_IMAGE_BYTES)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}

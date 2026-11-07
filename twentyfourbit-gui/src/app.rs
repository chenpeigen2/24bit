//! The window: navigation, lists, and the transport bar.
//!
//! All I/O lives in [`crate::backend`]; this module only paints and forwards
//! user intent. Anything that could block (network, cache) is a request, and
//! anything slow to read (audio state) is a snapshot polled each frame.

use std::path::PathBuf;
use std::time::Duration;

use twentyfourbit_core::{Quality, SearchItem, TrackDetail, WeeklyCharts};
use twentyfourbit_player::{Engine, Phase};

use crate::backend::{Backend, Event, LibraryRow, Request};
use crate::covers::{CoverState, Covers};
use crate::format::{format_bytes, format_duration, format_progress};
use crate::theme;

/// Which page is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// Search results.
    Search,
    /// Weekly charts.
    Charts,
    /// Cached tracks.
    Library,
    /// Settings and account.
    Settings,
}

/// Transient UI state.
pub struct App {
    backend: Backend,
    engine: Option<Engine>,
    covers: Covers,

    page: Page,
    query: String,
    last_query: String,
    busy: Option<&'static str>,

    results: Vec<SearchItem>,
    charts: WeeklyCharts,
    library: Vec<LibraryRow>,
    cache_files: usize,
    cache_bytes: u64,
    account: Option<String>,

    quality: Quality,
    progress: Option<(u64, Option<u64>)>,
    status: Option<(String, bool)>,

    now_playing: Option<TrackDetail>,
    lyrics: crate::lyrics::Lyrics,
    show_lyrics: bool,
    export_dir: PathBuf,
    cjk_font: Option<String>,
    font_warning: Option<String>,

    login_account: String,
    login_password: String,
}

impl App {
    /// Build the app, wiring the worker thread and audio engine.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_context(&cc.egui_ctx)
    }

    /// Build the app against an explicit context.
    ///
    /// Kept separate from [`App::new`] so tests can drive the UI without an
    /// `eframe::CreationContext` (which cannot be constructed outside eframe).
    pub fn with_context(ctx: &egui::Context) -> Self {
        let cjk = theme::install_fonts(ctx);
        theme::apply(ctx);

        let cfg = twentyfourbit_core::Config::load().unwrap_or_default();
        let quality = cfg.quality;

        let mut builder = twentyfourbit_core::Client::builder()
            .ip_placeholder(cfg.ip_placeholder.clone());
        // Debug/self-host override, matching the CLI's flag.
        if let Ok(url) = std::env::var("TWENTYFOURBIT_BASE_URL") {
            builder = builder.base_url(url);
        }
        if let Some(token) = &cfg.token {
            builder = builder.token(token.clone());
        }
        let client = builder.build().expect("cannot build HTTP client");

        let cache = twentyfourbit_core::Cache::default_cache();
        let backend = Backend::start(client, cache);
        backend.send(Request::Charts);
        backend.send(Request::Library);
        backend.send(Request::WhoAmI);

        let engine = Engine::start().ok();

        let font_warning = if cjk.is_none() {
            Some(
                "No CJK font found — Chinese titles will not render. \
                 Install one (e.g. Noto Sans CJK) and restart."
                    .to_owned(),
            )
        } else {
            None
        };

        Self {
            backend,
            engine,
            covers: Covers::new(ctx.clone()),
            page: Page::Search,
            query: String::new(),
            last_query: String::new(),
            busy: None,
            results: Vec::new(),
            charts: WeeklyCharts::default(),
            library: Vec::new(),
            cache_files: 0,
            cache_bytes: 0,
            account: None,
            quality,
            progress: None,
            status: None,
            now_playing: None,
            lyrics: crate::lyrics::Lyrics::default(),
            show_lyrics: true,
            export_dir: default_export_dir(),
            cjk_font: cjk,
            font_warning,
            login_account: String::new(),
            login_password: String::new(),
        }
    }

    fn drain_events(&mut self) {
        for event in self.backend.poll() {
            match event {
                Event::Busy(what) => {
                    self.busy = Some(what);
                    self.progress = None;
                }
                Event::Search(items) => {
                    self.results = items;
                    self.busy = None;
                }
                Event::Charts(charts) => {
                    self.charts = charts;
                    self.busy = None;
                }
                Event::Prepared { path, detail, .. } => {
                    self.busy = None;
                    self.progress = None;
                    self.adopt_track(detail, path);
                }
                Event::Exported { path, title } => {
                    self.busy = None;
                    self.progress = None;
                    self.status = Some((
                        format!("已导出 {title} → {}", path.display()),
                        false,
                    ));
                }
                Event::Progress { done, total } => self.progress = Some((done, total)),
                Event::Library(rows) => {
                    self.library = rows;
                    self.busy = None;
                }
                Event::CacheStats { files, bytes } => {
                    self.cache_files = files;
                    self.cache_bytes = bytes;
                }
                Event::Account(who) => self.account = who,
                Event::Error(message) => {
                    self.busy = None;
                    self.progress = None;
                    self.status = Some((message, true));
                }
            }
        }
    }

    /// Take ownership of a prepared track: parse its lyrics, start its cover
    /// download, and hand the file to the audio engine.
    ///
    /// Public because it is the seam the backend's `Prepared` event lands on,
    /// and the only way a headless test can put the app into a playing state.
    pub fn adopt_track(&mut self, detail: TrackDetail, path: PathBuf) {
        self.lyrics = detail
            .lrc
            .as_deref()
            .map(crate::lyrics::Lyrics::parse)
            .unwrap_or_default();
        // Lyrics only make sense when there are some.
        self.show_lyrics = !self.lyrics.is_empty();

        self.covers.request(&detail.cover);
        self.status = Some((format!("正在播放 {}", detail.name), false));

        if let Some(engine) = &self.engine {
            if path.exists() {
                engine.play(path);
            }
        } else {
            self.status = Some(("没有可用的音频设备".into(), true));
        }

        self.now_playing = Some(detail);
    }

    /// Lyrics currently loaded, for tests and for the lyrics panel.
    pub fn lyrics(&self) -> &crate::lyrics::Lyrics {
        &self.lyrics
    }

    /// Title of the track currently loaded, if any.
    pub fn now_playing_title(&self) -> Option<&str> {
        self.now_playing.as_ref().map(|d| d.name.as_str())
    }

    fn prepare(&self, id: &str) {
        self.backend.send(Request::Prepare {
            id: id.to_owned(),
            quality: self.quality,
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("nav")
            .exact_size(220.0)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("24bit")
                        .size(26.0)
                        .strong()
                        .color(theme::ACCENT),
                );
                ui.label(egui::RichText::new("Hi-Res 无损音乐").size(12.0).color(theme::TEXT_DIM));
                ui.add_space(18.0);

                for (page, label) in [
                    (Page::Search, "搜索"),
                    (Page::Charts, "榜单"),
                    (Page::Library, "音乐库"),
                    (Page::Settings, "设置"),
                ] {
                    let selected = self.page == page;
                    let text = egui::RichText::new(label).size(15.0);
                    let button = egui::Button::new(text)
                        .fill(if selected { theme::ACCENT_SOFT } else { theme::SURFACE })
                        .min_size(egui::vec2(190.0, 34.0));

                    if ui.add(button).clicked() {
                        self.page = page;
                        if page == Page::Library {
                            self.backend.send(Request::Library);
                        }
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} 个文件 · {}",
                            self.cache_files,
                            format_bytes(self.cache_bytes)
                        ))
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                    );
                    ui.label(
                        egui::RichText::new(match &self.account {
                            Some(a) => format!("已登录 · {a}"),
                            None => "游客".to_owned(),
                        })
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                    );
                    if let Some(font) = &self.cjk_font {
                        ui.label(
                            egui::RichText::new(format!("字体 · {font}"))
                                .size(10.0)
                                .color(theme::TEXT_DIM),
                        );
                    }
                });
            });
    }

    /// Draw a cover at `size`, falling back to a placeholder while it loads.
    fn show_cover(&mut self, url: &str, ui: &mut egui::Ui) -> Option<()> {
        if url.is_empty() {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 8.0, theme::SURFACE_HOVER);
            return None;
        }
        self.covers.request(url);
        match self.covers.get(url) {
            Some(CoverState::Ready(texture)) => {
                ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(egui::vec2(56.0, 56.0))
                        .corner_radius(8.0),
                );
                Some(())
            }
            _ => {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 8.0, theme::SURFACE_HOVER);
                None
            }
        }
    }

    /// Right-hand lyrics panel: highlights the line being sung and lets a click
    /// jump to it.
    fn lyrics_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_lyrics || self.lyrics.is_empty() {
            return;
        }

        let position = self
            .engine
            .as_ref()
            .and_then(|e| e.snapshot().transport)
            .map(|t| t.position())
            .unwrap_or_default();
        let current = self.lyrics.index_at(position);

        egui::Panel::right("lyrics")
            .exact_size(300.0)
            .resizable(true)
            .frame(
                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("歌词")
                        .size(14.0)
                        .strong()
                        .color(theme::TEXT_DIM),
                );
                ui.add_space(8.0);

                let mut seek_to: Option<Duration> = None;

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, line) in self.lyrics.lines.iter().enumerate() {
                            let is_current = Some(i) == current;
                            let colour = if is_current { theme::ACCENT } else { theme::TEXT_DIM };
                            let size = if is_current { 16.0 } else { 14.0 };

                            let response = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&line.text).size(size).color(colour),
                                )
                                .sense(egui::Sense::click()),
                            );

                            if is_current {
                                // Keep the sung line centred as playback moves.
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() {
                                seek_to = Some(line.at);
                            }
                            ui.add_space(6.0);
                        }
                    });

                if let Some(target) = seek_to {
                    if let Some(engine) = &self.engine {
                        engine.seek(target);
                    }
                }
            });
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.engine.as_ref().map(|e| e.snapshot());

        egui::Panel::bottom("transport")
            .exact_size(96.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                let Some(snapshot) = snapshot else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("没有可用的音频输出设备")
                                .color(theme::TEXT_DIM),
                        );
                    });
                    return;
                };

                let Some(transport) = &snapshot.transport else {
                    // A track may be loaded but unable to play (no device, or a
                    // file that failed to open). Showing the title anyway beats
                    // dropping straight back to an empty prompt.
                    if let Some(detail) = self.now_playing.clone() {
                        ui.horizontal(|ui| {
                            if self.show_cover(&detail.cover, ui).is_none() {
                                // placeholder is drawn inside show_cover
                            }
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&detail.name).size(16.0).strong());
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        detail.player, detail.quality
                                    ))
                                    .size(12.0)
                                    .color(theme::TEXT_DIM),
                                );
                            });
                        });
                        return;
                    }
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("选一首歌开始播放")
                                .size(14.0)
                                .color(theme::TEXT_DIM),
                        );
                    });
                    return;
                };

                let playing = transport.phase() == Phase::Playing;

                ui.horizontal(|ui| {
                    // Cover art for the track that is playing.
                    if let Some(cover) = self.now_playing.as_ref().map(|d| d.cover.clone()) {
                        self.show_cover(&cover, ui);
                    }

                    // Title block.
                    ui.vertical(|ui| {
                        let title = self
                            .now_playing
                            .as_ref()
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| "未知曲目".into());
                        let artist = self
                            .now_playing
                            .as_ref()
                            .map(|d| d.player.clone())
                            .unwrap_or_default();
                        ui.label(egui::RichText::new(title).size(16.0).strong());
                        ui.label(egui::RichText::new(artist).size(12.0).color(theme::TEXT_DIM));
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Volume.
                        let mut volume = transport.volume();
                        ui.add(
                            egui::Slider::new(&mut volume, 0.0..=1.0)
                                .show_value(false)
                                .trailing_fill(true),
                        )
                        .on_hover_text("音量");
                        if (volume - transport.volume()).abs() > f32::EPSILON {
                            if let Some(engine) = &self.engine {
                                engine.set_volume(volume);
                            }
                        }
                        ui.add_space(12.0);

                        // Play / pause.
                        let label = if playing { "⏸" } else { "▶" };
                        if ui
                            .add(egui::Button::new(egui::RichText::new(label).size(18.0)))
                            .clicked()
                        {
                            if let Some(engine) = &self.engine {
                                if playing {
                                    engine.pause();
                                } else {
                                    engine.resume();
                                }
                            }
                        }

                        // Time readout.
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {}",
                                format_duration(transport.position()),
                                format_duration(transport.duration())
                            ))
                            .size(12.0)
                            .color(theme::TEXT_DIM),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{}kHz · {}",
                                snapshot.output_rate / 1000,
                                if snapshot.resampling { "重采样" } else { "直通" }
                            ))
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                        );

                        if !self.lyrics.is_empty()
                            && ui
                                .selectable_label(self.show_lyrics, "歌词")
                                .clicked()
                        {
                            self.show_lyrics = !self.show_lyrics;
                        }
                    });
                });

                ui.add_space(6.0);

                // Seek bar.
                let mut progress = transport.progress() as f32;
                let response = ui.add(
                    egui::Slider::new(&mut progress, 0.0..=1.0)
                        .show_value(false)
                        .trailing_fill(true),
                );
                if response.changed() {
                    if let Some(engine) = &self.engine {
                        let target = transport.duration().mul_f64(progress as f64);
                        engine.seek(target);
                    }
                }
            });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status")
            .exact_size(28.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(what) = self.busy {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(format!("{what}…"))
                                .size(12.0)
                                .color(theme::TEXT_DIM),
                        );
                        if let Some((done, total)) = self.progress {
                            if let Some(total) = total.filter(|t| *t > 0) {
                                let fraction = done as f64 / total as f64;
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {}/{}",
                                        format_progress(fraction),
                                        format_bytes(done),
                                        format_bytes(total)
                                    ))
                                    .size(12.0)
                                    .color(theme::TEXT_DIM),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format_bytes(done))
                                        .size(12.0)
                                        .color(theme::TEXT_DIM),
                                );
                            }
                        }
                    } else if let Some((message, is_error)) = &self.status {
                        let colour = if *is_error { theme::DANGER } else { theme::TEXT_DIM };
                        ui.label(egui::RichText::new(message).size(12.0).color(colour));
                    }

                    if let Some(warning) = &self.font_warning {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(warning)
                                    .size(11.0)
                                    .color(theme::DANGER),
                            );
                        });
                    }
                });
            });
    }

    fn search_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("歌曲 / 歌手")
                    .desired_width(360.0),
            );
            let submitted =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if ui.button("搜索").clicked() || submitted {
                let q = self.query.trim().to_owned();
                if !q.is_empty() {
                    self.last_query = q.clone();
                    self.backend.send(Request::Search(q));
                }
            }
        });

        ui.add_space(10.0);

        if self.results.is_empty() {
            ui.label(
                egui::RichText::new(if self.last_query.is_empty() {
                    "输入关键词开始搜索".to_owned()
                } else {
                    format!("没有找到「{}」", self.last_query)
                })
                .color(theme::TEXT_DIM),
            );
            return;
        }

        ui.label(
            egui::RichText::new(format!("{} 个结果", self.results.len()))
                .size(12.0)
                .color(theme::TEXT_DIM),
        );
        ui.add_space(4.0);

        let mut to_prepare: Option<String> = None;
        let mut to_export: Option<String> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &self.results {
                self.covers.request(&item.cover);

                let row = ui.horizontal(|ui| {
                    // Cover thumbnail.
                    match self.covers.get(&item.cover) {
                        Some(CoverState::Ready(texture)) => {
                            ui.add(
                                egui::Image::new(texture)
                                    .fit_to_exact_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(6.0),
                            );
                        }
                        _ => {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 6.0, theme::SURFACE_HOVER);
                        }
                    }

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&item.name).size(15.0).strong());
                        ui.label(
                            egui::RichText::new(format!("{} · {}", item.player, item.album))
                                .size(12.0)
                                .color(theme::TEXT_DIM),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("播放").clicked() {
                            to_prepare = Some(item.id.clone());
                        }
                        if ui.button("下载").on_hover_text(self.export_dir.display().to_string()).clicked() {
                            to_export = Some(item.id.clone());
                        }
                    });
                });

                if row
                    .response
                    .interact(egui::Sense::click())
                    .double_clicked()
                {
                    to_prepare = Some(item.id.clone());
                }

                ui.separator();
            }
        });

        if let Some(id) = to_prepare {
            self.prepare(&id);
        } else if let Some(id) = to_export {
            self.backend.send(Request::Export {
                id,
                quality: self.quality,
                destination: self.export_dir.clone(),
            });
        }
    }

    fn charts_page(&mut self, ui: &mut egui::Ui) {
        let charts = self.charts.clone();
        if charts.songs.is_empty() {
            ui.label(egui::RichText::new("暂无榜单数据").color(theme::TEXT_DIM));
            return;
        }

        ui.columns(2, |columns| {
            columns[0].label(egui::RichText::new("本周歌曲榜").size(16.0).strong());
            columns[0].add_space(6.0);
            for (i, row) in charts.songs.iter().enumerate() {
                columns[0].horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:>2}", i + 1))
                            .color(if i < 3 { theme::ACCENT } else { theme::TEXT_DIM }),
                    );
                    if ui.link(&row.name).clicked() {
                        self.query = row.name.clone();
                        self.last_query = row.name.clone();
                        self.page = Page::Search;
                        self.backend.send(Request::Search(row.name.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(row.plays.to_string())
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                    });
                });
            }

            columns[1].label(egui::RichText::new("本周歌手榜").size(16.0).strong());
            columns[1].add_space(6.0);
            for (i, row) in charts.artists.iter().enumerate() {
                columns[1].horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:>2}", i + 1))
                            .color(if i < 3 { theme::ACCENT } else { theme::TEXT_DIM }),
                    );
                    if ui.link(&row.name).clicked() {
                        self.query = row.name.clone();
                        self.last_query = row.name.clone();
                        self.page = Page::Search;
                        self.backend.send(Request::Search(row.name.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(row.plays.to_string())
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                    });
                });
            }
        });
    }

    fn library_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("已下载").size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("刷新").clicked() {
                    self.backend.send(Request::Library);
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("清空缓存").color(theme::DANGER),
                    ))
                    .clicked()
                {
                    self.backend.send(Request::ClearCache);
                }
            });
        });
        ui.add_space(8.0);

        if self.library.is_empty() {
            ui.label(
                egui::RichText::new("还没有缓存的曲目")
                    .color(theme::TEXT_DIM),
            );
            return;
        }

        let mut to_open: Option<PathBuf> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for row in &self.library {
                ui.horizontal(|ui| {
                    ui.label(&row.name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format_bytes(row.bytes))
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                        if ui.button("播放").clicked() {
                            to_open = Some(row.path.clone());
                        }
                    });
                });
                ui.separator();
            }
        });

        if let Some(path) = to_open {
            if let Some(engine) = &self.engine {
                engine.play(path);
            }
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("音质偏好").size(16.0).strong());
        ui.horizontal(|ui| {
            for (q, label) in [
                (Quality::Master, "至臻母带 (24bit/192kHz)"),
                (Quality::Surround, "高清环绕声"),
            ] {
                if ui
                    .selectable_label(self.quality == q, label)
                    .clicked()
                {
                    self.quality = q;
                    let mut cfg = twentyfourbit_core::Config::load().unwrap_or_default();
                    cfg.quality = q;
                    let _ = cfg.save();
                }
            }
        });

        ui.add_space(14.0);
        ui.label(egui::RichText::new("缓存").size(16.0).strong());
        ui.label(
            egui::RichText::new(format!(
                "{} 个文件 · {}",
                self.cache_files,
                format_bytes(self.cache_bytes)
            ))
            .color(theme::TEXT_DIM),
        );
        ui.label(
            egui::RichText::new(
                twentyfourbit_core::Cache::default_cache()
                    .root()
                    .display()
                    .to_string(),
            )
            .size(11.0)
            .color(theme::TEXT_DIM),
        );
        if ui.button("清空缓存").clicked() {
            self.backend.send(Request::ClearCache);
        }

        ui.add_space(14.0);
        ui.label(egui::RichText::new("下载目录").size(16.0).strong());
        ui.label(
            egui::RichText::new(self.export_dir.display().to_string())
                .size(11.0)
                .color(theme::TEXT_DIM),
        );

        ui.add_space(14.0);
        ui.label(egui::RichText::new("账号").size(16.0).strong());
        ui.label(
            egui::RichText::new(match &self.account {
                Some(a) => format!("已登录：{a}"),
                None => "游客（每天可下载数首）".to_owned(),
            })
            .color(theme::TEXT_DIM),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.login_account)
                    .hint_text("账号")
                    .desired_width(160.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.login_password)
                    .hint_text("密码")
                    .password(true)
                    .desired_width(160.0),
            );
            if ui.button("登录").clicked()
                && !self.login_account.is_empty()
                && !self.login_password.is_empty()
            {
                self.backend.send(Request::Login {
                    account: self.login_account.clone(),
                    password: self.login_password.clone(),
                });
            }
        });

        ui.add_space(14.0);
        ui.label(egui::RichText::new("关于").size(16.0).strong());
        ui.label(
            egui::RichText::new(format!(
                "24bit {} · 非官方客户端 · MIT OR Apache-2.0",
                env!("CARGO_PKG_VERSION")
            ))
            .size(11.0)
            .color(theme::TEXT_DIM),
        );
        if let Some(engine) = &self.engine {
            let snapshot = engine.snapshot();
            ui.label(
                egui::RichText::new(format!(
                    "输出设备 {}kHz · 源 {}kHz · {}",
                    snapshot.output_rate / 1000,
                    snapshot.source_rate / 1000,
                    if snapshot.resampling { "重采样中" } else { "直通" }
                ))
                .size(11.0)
                .color(theme::TEXT_DIM),
            );
        }
        let _ = Duration::ZERO;
    }
}

impl App {
    /// Paint one frame. Split out from [`eframe::App::ui`] so headless tests can
    /// drive exactly the same code path.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.drain_events();
        self.covers.poll(&ctx);

        // Keep the transport bar live while audio is playing.
        if self
            .engine
            .as_ref()
            .and_then(|e| e.snapshot().transport)
            .map(|t| t.phase() == Phase::Playing)
            .unwrap_or(false)
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        self.sidebar(ui);
        self.status_bar(ui);
        self.transport(ui);
        self.lyrics_panel(ui);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| match self.page {
                Page::Search => self.search_page(ui),
                Page::Charts => self.charts_page(ui),
                Page::Library => self.library_page(ui),
                Page::Settings => self.settings_page(ui),
            });
    }

    /// Which page is showing (used by tests and by deep links).
    pub fn page(&self) -> Page {
        self.page
    }

    /// Switch page programmatically.
    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        if page == Page::Library {
            self.backend.send(Request::Library);
        }
    }

    /// Number of search results currently held.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(engine) = &self.engine {
            engine.shutdown();
        }
    }
}

/// Where "download to disk" puts files by default.
fn default_export_dir() -> PathBuf {
    dirs::audio_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("24bit")
}

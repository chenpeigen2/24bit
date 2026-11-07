//! The window.
//!
//! ```text
//! ┌─────────────────────────────────────────────── header (drag) ────┐
//! │ sidebar │              page content                │   lyrics     │
//! ├───────────────────────────────────────────────────────────────────┤
//! │                          transport                                │
//! ├───────────────────────────────────────────────────────────────────┤
//! │                          status                                   │
//! └───────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Everything slow lives in [`crate::backend`]; this module paints and forwards
//! intent. Audio state is a per-frame snapshot, so a stalled decoder can never
//! freeze the window.
//!
//! List rows are painted by hand rather than composed from nested layouts: the
//! row geometry is then exact, hit areas are explicit, and a click on "download"
//! cannot be mistaken for a click on the row.

use std::path::PathBuf;
use std::time::Duration;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use twentyfourbit_core::{Quality, SearchItem, TrackDetail, WeeklyCharts};
use twentyfourbit_player::{Engine, Phase};

use crate::backend::{Backend, Event, LibraryRow, Request};
use crate::covers::{CoverState, Covers};
use crate::format::{format_bytes, format_duration};
use crate::icons::{self, Icon, Style};
use crate::theme::*;

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

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Search => "搜索",
            Page::Charts => "本周榜单",
            Page::Library => "音乐库",
            Page::Settings => "设置",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Page::Search => "同时检索两个曲库，结果自动合并去重",
            Page::Charts => "24bit.net 每周播放统计",
            Page::Library => "已缓存的曲目，可离线播放",
            Page::Settings => "音质、缓存与账号",
        }
    }
}

/// Transient UI state.
pub struct App {
    backend: Backend,
    engine: Option<Engine>,
    covers: Covers,

    page: Page,
    query: String,
    last_query: String,
    debounce: crate::debounce::Debounce,
    busy: Option<&'static str>,
    charts_requested: bool,

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

    /// Build against an explicit context.
    ///
    /// Separate from [`App::new`] so headless tests drive the same code path
    /// without an `eframe::CreationContext`.
    pub fn with_context(ctx: &egui::Context) -> Self {
        let cjk = install_fonts(ctx);
        apply(ctx);

        let cfg = twentyfourbit_core::Config::load().unwrap_or_default();
        let quality = cfg.quality;

        let mut builder =
            twentyfourbit_core::Client::builder().ip_placeholder(cfg.ip_placeholder.clone());
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
        // Charts come from the network and the site can take several seconds, so
        // they are fetched when the page is first opened rather than at launch.
        backend.send(Request::Library);
        backend.send(Request::WhoAmI);

        let engine = Engine::start().ok();

        let font_warning = cjk.is_none().then(|| {
            "未找到中文字体，标题无法正常显示 · 请安装 Noto Sans CJK 后重启".to_owned()
        });

        Self {
            backend,
            engine,
            covers: Covers::new(ctx.clone()),
            page: Page::Search,
            query: String::new(),
            last_query: String::new(),
            debounce: crate::debounce::Debounce::new(0.3),
            busy: None,
            charts_requested: false,
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

    // --------------------------------------------------------------- state

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
                    self.status = Some((format!("已保存 {title} · {}", path.display()), false));
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

    /// Take ownership of a prepared track.
    ///
    /// Public because it is the seam the backend's `Prepared` event lands on, and
    /// the only way a headless test can put the app into a playing state.
    pub fn adopt_track(&mut self, detail: TrackDetail, path: PathBuf) {
        self.lyrics = detail
            .lrc
            .as_deref()
            .map(crate::lyrics::Lyrics::parse)
            .unwrap_or_default();
        self.show_lyrics = !self.lyrics.is_empty();

        self.covers.request(&detail.cover);
        self.status = Some((format!("正在播放 {}", detail.name), false));

        match &self.engine {
            Some(engine) if path.exists() => engine.play(path),
            Some(_) => {}
            None => self.status = Some(("没有可用的音频设备".into(), true)),
        }

        self.now_playing = Some(detail);
    }

    // Seeds for tests and headless screenshot rendering. They bypass the
    // backend so a page can be inspected without any network or audio device.
    /// Install search results directly.
    pub fn seed_results(&mut self, results: Vec<SearchItem>) {
        self.last_query = "富士山下".to_owned();
        self.results = results;
    }

    /// Install charts directly.
    pub fn seed_charts(&mut self, charts: WeeklyCharts) {
        self.charts_requested = true;
        self.charts = charts;
    }

    /// Install library rows directly.
    pub fn seed_library(&mut self, rows: Vec<LibraryRow>, bytes: u64) {
        self.cache_bytes = bytes;
        self.cache_files = rows.len();
        self.library = rows;
    }

    /// Which page is showing.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Switch page programmatically.
    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        match page {
            Page::Library => self.backend.send(Request::Library),
            Page::Charts if self.charts.songs.is_empty() && !self.charts_requested => {
                self.charts_requested = true;
                self.backend.send(Request::Charts);
            }
            _ => {}
        }
    }

    /// Number of search results currently held.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Lyrics currently loaded.
    pub fn lyrics(&self) -> &crate::lyrics::Lyrics {
        &self.lyrics
    }

    /// Title of the loaded track, if any.
    pub fn now_playing_title(&self) -> Option<&str> {
        self.now_playing.as_ref().map(|d| d.name.as_str())
    }

    fn prepare(&self, id: &str) {
        self.backend.send(Request::Prepare {
            id: id.to_owned(),
            quality: self.quality,
        });
    }

    // -------------------------------------------------------------- chrome

    /// Custom title bar: the macOS traffic lights sit over the left padding and
    /// dragging the bar moves the window.
    fn header(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_H), Sense::drag());
        let painter = ui.painter().clone();

        painter.rect_filled(rect, 0.0, PANEL);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.bottom() - 1.0),
                Pos2::new(rect.right(), rect.bottom()),
            ),
            0.0,
            BORDER,
        );

        if response.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // Clearance for the traffic lights drawn by the OS.
        painter.text(
            Pos2::new(rect.left() + 84.0, rect.center().y),
            Align2::LEFT_CENTER,
            "24bit",
            FontId::proportional(15.0),
            TEXT,
        );
        painter.text(
            Pos2::new(rect.left() + 84.0 + 44.0, rect.center().y),
            Align2::LEFT_CENTER,
            "HI-RES",
            FontId::proportional(9.0),
            TEXT_FAINT,
        );

        let brand_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 84.0, rect.center().y - 10.0),
            Pos2::new(rect.left() + 190.0, rect.center().y + 10.0),
        );
        expose(ui, brand_rect, "24bit");

        // Account chip.
        let label = self.account.clone().unwrap_or_else(|| "游客".to_owned());
        let chip_w = 28.0 + label.chars().count() as f32 * 7.0;
        let chip = Rect::from_center_size(
            Pos2::new(rect.right() - 20.0 - chip_w / 2.0, rect.center().y),
            Vec2::new(chip_w, 22.0),
        );
        painter.rect_filled(chip, 11.0, CARD);
        painter.text(
            chip.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(T_FAINT),
            TEXT_MUTED,
        );
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("nav")
            .exact_size(SIDEBAR_W)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(PANEL))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.right() - 1.0, rect.top()),
                        Pos2::new(rect.right(), rect.bottom()),
                    ),
                    0.0,
                    BORDER,
                );

                ui.add_space(20.0);
                ui.painter().text(
                    Pos2::new(rect.left() + 18.0, rect.top() + 20.0),
                    Align2::LEFT_TOP,
                    "浏览",
                    FontId::proportional(10.0),
                    TEXT_FAINT,
                );
                ui.add_space(24.0);

                let mut switch_to = None;
                for (page, icon) in [
                    (Page::Search, Icon::Search),
                    (Page::Charts, Icon::Music),
                    (Page::Library, Icon::Library),
                    (Page::Settings, Icon::Gear),
                ] {
                    if self.nav_item(ui, page, icon) {
                        switch_to = Some(page);
                    }
                }
                if let Some(page) = switch_to {
                    self.set_page(page);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(16.0);
                    ui.painter().text(
                        Pos2::new(rect.left() + 18.0, ui.cursor().top() - 14.0),
                        Align2::LEFT_TOP,
                        format!(
                            "{} 个文件 · {}",
                            self.cache_files,
                            format_bytes(self.cache_bytes)
                        ),
                        FontId::proportional(T_FAINT),
                        TEXT_FAINT,
                    );
                    ui.add_space(26.0);
                });
            });
    }

    /// One sidebar row. Returns true when clicked.
    fn nav_item(&mut self, ui: &mut egui::Ui, page: Page, icon: Icon) -> bool {
        let height = 36.0;
        let full = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(full, height + 2.0), Sense::click());

        let pill = Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, rect.top() + 1.0),
            Pos2::new(rect.right() - 10.0, rect.bottom() - 1.0),
        );

        let active = self.page == page;
        let painter = ui.painter().clone();

        if active {
            icons::card(&painter, pill, RADIUS_SM, ACCENT_SOFT, None);
        } else if response.hovered() {
            icons::card(&painter, pill, RADIUS_SM, CARD, None);
        }

        let fg = if active {
            TEXT
        } else if response.hovered() {
            TEXT
        } else {
            TEXT_MUTED
        };

        icons::paint(
            &painter,
            Rect::from_center_size(Pos2::new(pill.left() + 22.0, pill.center().y), Vec2::splat(18.0)),
            icon,
            fg,
        );

        let label = match page {
            Page::Search => "搜索",
            Page::Charts => "榜单",
            Page::Library => "音乐库",
            Page::Settings => "设置",
        };
        painter.text(
            Pos2::new(pill.left() + 42.0, pill.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(T_ITEM),
            fg,
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, active, label)
        });

        response.clicked()
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status")
            .exact_size(STATUS_H)
            .frame(egui::Frame::NONE.fill(PANEL))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top()),
                        Pos2::new(rect.right(), rect.top() + 1.0),
                    ),
                    0.0,
                    BORDER,
                );

                let mut x = rect.left() + GUTTER;
                let y = rect.center().y;

                if let Some(what) = self.busy {
                    // A rotating arc, because a static "loading…" reads as stuck
                    // when the site takes several seconds to answer.
                    spinner(&ui.painter().clone(), Pos2::new(x + 5.0, y), 5.0, TEXT_MUTED, ui.input(|i| i.time));
                    x += 16.0;

                    let text = match self.progress {
                        Some((done, Some(total))) if total > 0 => format!(
                            "{} · {} / {}",
                            what,
                            format_bytes(done),
                            format_bytes(total)
                        ),
                        Some((done, None)) => format!("{what} · {}", format_bytes(done)),
                        _ => format!("{what}…"),
                    };
                    ui.painter().text(
                        Pos2::new(x, y),
                        Align2::LEFT_CENTER,
                        text,
                        FontId::proportional(T_FAINT),
                        TEXT_MUTED,
                    );
                    x += 300.0;

                    // A thin progress line makes a long download legible.
                    if let Some((done, Some(total))) = self.progress {
                        if total > 0 {
                            let track = Rect::from_min_max(
                                Pos2::new(x, y - 1.5),
                                Pos2::new(x + 160.0, y + 1.5),
                            );
                            ui.painter().rect_filled(track, 2.0, BORDER);
                            let w = 160.0 * (done as f32 / total as f32).clamp(0.0, 1.0);
                            ui.painter().rect_filled(
                                Rect::from_min_max(track.min, Pos2::new(track.left() + w, track.max.y)),
                                2.0,
                                ACCENT,
                            );
                        }
                    }
                } else if let Some((message, is_error)) = &self.status {
                    let colour = if *is_error { DANGER } else { TEXT_MUTED };
                    ui.painter().text(
                        Pos2::new(x, y),
                        Align2::LEFT_CENTER,
                        message,
                        FontId::proportional(T_FAINT),
                        colour,
                    );
                }

                if let Some(warning) = &self.font_warning {
                    ui.painter().text(
                        Pos2::new(rect.right() - GUTTER, y),
                        Align2::RIGHT_CENTER,
                        warning,
                        FontId::proportional(T_FAINT),
                        DANGER,
                    );
                } else {
                    ui.painter().text(
                        Pos2::new(rect.right() - GUTTER, y),
                        Align2::RIGHT_CENTER,
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                        FontId::proportional(T_FAINT),
                        Color32::from_rgb(72, 72, 84),
                    );
                }
            });
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.engine.as_ref().map(|e| e.snapshot());

        egui::Panel::bottom("transport")
            .exact_size(TRANSPORT_H)
            .frame(egui::Frame::NONE.fill(PANEL))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter().clone();
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top()),
                        Pos2::new(rect.right(), rect.top() + 1.0),
                    ),
                    0.0,
                    BORDER,
                );

                let Some(snapshot) = snapshot else {
                    self.empty_transport(ui, rect, "没有可用的音频输出设备");
                    return;
                };

                let transport = snapshot.transport.clone();
                let cover = self.now_playing.as_ref().map(|d| d.cover.clone());
                // With nothing loaded the bar still has a job: tell the user how
                // to start, rather than showing an empty slot.
                let (title, artist) = match &self.now_playing {
                    Some(d) => (d.name.clone(), d.player.clone()),
                    None => ("选一首歌开始播放".to_owned(), String::new()),
                };
                let quality = self
                    .now_playing
                    .as_ref()
                    .map(|d| d.quality.clone())
                    .unwrap_or_default();

                // Geometry: progress row on top, controls row beneath.
                let inner = rect.shrink2(Vec2::new(GUTTER, 12.0));
                let progress_rect = Rect::from_min_max(
                    Pos2::new(inner.left(), inner.top()),
                    Pos2::new(inner.right(), inner.top() + 26.0),
                );
                let row = Rect::from_min_max(
                    Pos2::new(inner.left(), progress_rect.bottom() + 6.0),
                    Pos2::new(inner.right(), inner.bottom()),
                );

                // ---- progress row
                let mut seek_target = None;
                {
                    let position = transport
                        .as_ref()
                        .map(|t| t.position())
                        .unwrap_or_default();
                    let duration = transport
                        .as_ref()
                        .map(|t| t.duration())
                        .unwrap_or_default();
                    let fraction = if duration.as_secs_f64() > 0.0 {
                        (position.as_secs_f64() / duration.as_secs_f64()) as f32
                    } else {
                        0.0
                    };

                    painter.text(
                        Pos2::new(progress_rect.left(), progress_rect.top()),
                        Align2::LEFT_TOP,
                        format_duration(position),
                        FontId::proportional(T_FAINT),
                        TEXT_FAINT,
                    );
                    painter.text(
                        Pos2::new(progress_rect.right(), progress_rect.top()),
                        Align2::RIGHT_TOP,
                        format_duration(duration),
                        FontId::proportional(T_FAINT),
                        TEXT_FAINT,
                    );

                    let bar_rect = Rect::from_min_max(
                        Pos2::new(progress_rect.left(), progress_rect.bottom() - 10.0),
                        Pos2::new(progress_rect.right(), progress_rect.bottom()),
                    );
                    let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar_rect));
                    let (_, requested) = icons::seek_bar(&mut bar_ui, fraction, 10.0);
                    if let Some(t) = requested {
                        seek_target = Some(duration.mul_f64(t as f64));
                    }
                }
                if let Some(target) = seek_target {
                    if let Some(engine) = &self.engine {
                        engine.seek(target);
                    }
                }

                // ---- cover + title
                let cover_size = 46.0;
                let cover_rect = Rect::from_center_size(
                    Pos2::new(row.left() + cover_size / 2.0, row.center().y),
                    Vec2::splat(cover_size),
                );
                self.draw_cover(ui, cover.as_deref().unwrap_or(""), cover_rect);

                let text_left = cover_rect.right() + 12.0;
                painter.text(
                    Pos2::new(text_left, row.center().y - 9.0),
                    Align2::LEFT_CENTER,
                    &title,
                    FontId::proportional(T_ITEM),
                    TEXT,
                );
                let title_rect = Rect::from_min_max(
                    Pos2::new(text_left, row.center().y - 18.0),
                    Pos2::new(rect.center().x - 120.0, row.center().y),
                );
                expose(ui, title_rect, &title);
                painter.text(
                    Pos2::new(text_left, row.center().y + 10.0),
                    Align2::LEFT_CENTER,
                    if artist.is_empty() {
                        "从搜索或榜单开始".to_owned()
                    } else {
                        format!("{artist} · {quality}")
                    },
                    FontId::proportional(T_META),
                    TEXT_MUTED,
                );

                // ---- centre controls
                let cx = rect.center().x;
                let cy = row.center().y;
                let playing = transport
                    .as_ref()
                    .map(|t| t.phase() == Phase::Playing)
                    .unwrap_or(false);

                let play_rect = Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(40.0));
                let play_response = icons::button_at(ui, play_rect, if playing { Icon::Pause } else { Icon::Play }, Style::Accent);
                if play_response.clicked() {
                    if let Some(engine) = &self.engine {
                        if playing {
                            engine.pause();
                        } else {
                            engine.resume();
                        }
                    }
                }

                let prev_rect =
                    Rect::from_center_size(Pos2::new(cx - 56.0, cy), Vec2::splat(32.0));
                let restart = icons::button_at(ui, prev_rect, Icon::Previous, Style::Ghost);
                if restart.clicked() {
                    if let Some(engine) = &self.engine {
                        engine.seek(Duration::ZERO);
                        engine.resume();
                    }
                }

                let next_rect = Rect::from_center_size(Pos2::new(cx + 56.0, cy), Vec2::splat(32.0));
                icons::button_at(ui, next_rect, Icon::Next, Style::Ghost)
                    .on_hover_text("下一首（队列功能尚未实现）");

                // ---- right side: volume, lyrics, format
                let mut right = rect.right() - GUTTER;

                if !self.lyrics.is_empty() {
                    let rect_l = Rect::from_center_size(
                        Pos2::new(right - 14.0, cy),
                        Vec2::splat(28.0),
                    );
                    let shown = self.show_lyrics;
                    let response = icons::button_at(
                        ui,
                        rect_l,
                        Icon::Lyrics,
                        if shown { Style::Filled } else { Style::Ghost },
                    )
                    .on_hover_text(if shown { "隐藏歌词" } else { "显示歌词" });
                    if response.clicked() {
                        self.show_lyrics = !self.show_lyrics;
                    }
                    right -= 40.0;
                }

                let vol_rect = Rect::from_min_max(
                    Pos2::new(right - 92.0, cy - 8.0),
                    Pos2::new(right, cy + 8.0),
                );
                let mut volume = transport.as_ref().map(|t| t.volume()).unwrap_or(1.0);
                let mut vol_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(vol_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                vol_ui.spacing_mut().slider_width = 64.0;
                let vol_response = vol_ui.add(
                    egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false),
                );
                // A bare slider has no accessible name; screen readers would
                // announce "slider" and stop there.
                vol_response.widget_info(|| {
                    egui::WidgetInfo::slider(true, volume as f64, "音量")
                });
                let changed = vol_response.changed();
                if changed {
                    if let Some(engine) = &self.engine {
                        engine.set_volume(volume);
                    }
                }

                let vol_icon = Rect::from_center_size(
                    Pos2::new(right - 92.0 - 18.0, cy),
                    Vec2::splat(20.0),
                );
                icons::paint(ui.painter(), vol_icon, Icon::Volume, TEXT_FAINT);

                // Output format chip, far left of the right cluster.
                let chip = format!(
                    "{}kHz {}",
                    snapshot.output_rate / 1000,
                    if snapshot.resampling { "重采样" } else { "直通" }
                );
                ui.painter().text(
                    Pos2::new(right - 92.0 - 44.0, cy),
                    Align2::RIGHT_CENTER,
                    chip,
                    FontId::proportional(10.0),
                    Color32::from_rgb(86, 86, 100),
                );
            });
    }

    fn empty_transport(&self, ui: &mut egui::Ui, rect: Rect, message: &str) {
        let painter = ui.painter().clone();
        icons::paint(
            &painter,
            Rect::from_center_size(
                Pos2::new(rect.left() + GUTTER + 12.0, rect.center().y),
                Vec2::splat(20.0),
            ),
            Icon::Music,
            Color32::from_rgb(58, 58, 70),
        );
        painter.text(
            Pos2::new(rect.left() + GUTTER + 32.0, rect.center().y),
            Align2::LEFT_CENTER,
            message,
            FontId::proportional(T_BODY),
            TEXT_FAINT,
        );
        let message_rect = Rect::from_min_max(
            Pos2::new(rect.left() + GUTTER + 32.0, rect.center().y - 12.0),
            Pos2::new(rect.right() - GUTTER, rect.center().y + 12.0),
        );
        expose(ui, message_rect, message);
    }

    /// Draw a cover into `rect`, holding the layout with a placeholder while the
    /// image loads.
    fn draw_cover(&mut self, ui: &mut egui::Ui, url: &str, rect: Rect) {
        if !url.is_empty() {
            self.covers.request(url);
            if let Some(CoverState::Ready(texture)) = self.covers.get(url) {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                child.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(rect.size())
                        .corner_radius(RADIUS_SM as u8),
                );
                return;
            }
        }
        ui.painter().rect_filled(rect, RADIUS_SM, CARD_HOVER);
        icons::paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.42)),
            Icon::Music,
            Color32::from_rgb(96, 96, 104),
        );
    }

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
            .exact_size(304.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(PANEL))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top()),
                        Pos2::new(rect.left() + 1.0, rect.bottom()),
                    ),
                    0.0,
                    BORDER,
                );

                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.label(
                        egui::RichText::new("歌词")
                            .size(10.0)
                            .color(TEXT_FAINT),
                    );
                });
                ui.add_space(10.0);

                let mut seek_to = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, line) in self.lyrics.lines.iter().enumerate() {
                            let is_current = Some(i) == current;
                            let colour = if is_current { TEXT } else { TEXT_FAINT };
                            let size = if is_current { T_ITEM } else { T_BODY };

                            ui.horizontal(|ui| {
                                ui.add_space(GUTTER);
                                let response = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&line.text).size(size).color(colour),
                                    )
                                    .sense(Sense::click()),
                                );
                                if is_current {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    seek_to = Some(line.at);
                                }
                            });
                            ui.add_space(10.0);
                        }
                    });

                if let Some(target) = seek_to {
                    if let Some(engine) = &self.engine {
                        engine.seek(target);
                    }
                }
            });
    }

    // --------------------------------------------------------------- pages

    fn page_header(&self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(self.page.title())
                        .size(T_TITLE)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(self.page.subtitle())
                        .size(T_META)
                        .color(TEXT_FAINT),
                );
            });
        });
        ui.add_space(16.0);
    }

    fn search_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui);

        // ---- search field
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            let width = (ui.available_width() - GUTTER).min(560.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());

            let painter = ui.painter().clone();
            icons::card(&painter, rect, RADIUS_SM, CARD, Some(BORDER));
            icons::paint(
                &painter,
                Rect::from_center_size(
                    Pos2::new(rect.left() + 22.0, rect.center().y),
                    Vec2::splat(17.0),
                ),
                Icon::Search,
                TEXT_FAINT,
            );

            let inner = Rect::from_min_max(
                Pos2::new(rect.left() + 40.0, rect.top() + 7.0),
                Pos2::new(rect.right() - 12.0, rect.bottom() - 7.0),
            );
            let mut field_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let response = field_ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("歌曲 / 歌手")
                    .desired_width(inner.width())
                    .frame(egui::Frame::NONE)
                    .font(FontId::proportional(T_BODY)),
            );

            let now = ui.input(|i| i.time);
            let submitted =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if response.changed() {
                self.debounce.note_edit(now);
            }
            if self.debounce.is_pending() {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
            let due = self.debounce.poll(now);
            if submitted {
                self.debounce.cancel();
            }

            if submitted || due {
                let q = self.query.trim().to_owned();
                if !q.is_empty() && q != self.last_query {
                    self.last_query = q.clone();
                    self.backend.send(Request::Search(q));
                }
            }
        });

        ui.add_space(14.0);

        if self.results.is_empty() {
            empty_state(
                ui,
                Icon::Search,
                if self.last_query.is_empty() {
                    "输入关键词开始搜索"
                } else {
                    "没有找到匹配的歌曲"
                },
                if self.last_query.is_empty() {
                    "试试歌名或歌手，例如「富士山下」「陈奕迅」"
                } else {
                    "换个写法试试，或者直接浏览本周榜单"
                },
            );
            return;
        }

        // ---- result count
        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            ui.label(
                egui::RichText::new(format!("{} 个结果", self.results.len()))
                    .size(T_FAINT)
                    .color(TEXT_FAINT),
            );
        });
        ui.add_space(6.0);

        let mut to_prepare = None;
        let mut to_export = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let items = std::mem::take(&mut self.results);
                for (i, item) in items.iter().enumerate() {
                    match self.result_row(ui, item, i) {
                        RowAction::Play => to_prepare = Some(item.id.clone()),
                        RowAction::Export => to_export = Some(item.id.clone()),
                        RowAction::None => {}
                    }
                }
                self.results = items;
                ui.add_space(GUTTER);
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

    fn result_row(&mut self, ui: &mut egui::Ui, item: &SearchItem, index: usize) -> RowAction {
        let height = 68.0;
        let gutter = GUTTER;
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width() - gutter * 2.0, height + 2.0),
            Sense::hover(),
        );
        let rect = Rect::from_min_max(
            Pos2::new(rect.left() + gutter, rect.top()),
            Pos2::new(rect.right() + gutter, rect.bottom() - 2.0),
        );

        // Hover state is resolved on the body only, so the buttons keep their
        // own hit areas.
        let body = Rect::from_min_max(rect.min, Pos2::new(rect.right() - 104.0, rect.max.y));
        let hover = ui
            .interact(body, ui.scope_id().with(("row", index)), Sense::click())
            .hovered();
        let clicked = ui
            .interact(body, ui.scope_id().with(("row", index)), Sense::click())
            .clicked();

        let painter = ui.painter().clone();
        if hover {
            icons::card(&painter, rect, RADIUS, CARD, None);
        }

        let cover_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, rect.top() + 10.0),
            Pos2::new(rect.left() + 58.0, rect.bottom() - 10.0),
        );
        self.draw_cover(ui, &item.cover, cover_rect);

        ui.painter().text(
            Pos2::new(cover_rect.right() + 12.0, rect.center().y - 9.0),
            Align2::LEFT_CENTER,
            &item.name,
            FontId::proportional(T_ITEM),
            TEXT,
        );
        let title_rect = Rect::from_min_max(
            Pos2::new(cover_rect.right() + 12.0, rect.center().y - 18.0),
            Pos2::new(body.right(), rect.center().y),
        );
        expose(ui, title_rect, &item.name);
        ui.painter().text(
            Pos2::new(cover_rect.right() + 12.0, rect.center().y + 10.0),
            Align2::LEFT_CENTER,
            format!("{} · {}", item.player, item.album),
            FontId::proportional(T_META),
            TEXT_MUTED,
        );

        let play_rect = Rect::from_center_size(
            Pos2::new(rect.right() - 26.0, rect.center().y),
            Vec2::splat(34.0),
        );
        let play = icons::button_at(
            ui,
            play_rect,
            Icon::Play,
            if hover { Style::Accent } else { Style::Filled },
        );

        let download_rect = Rect::from_center_size(
            Pos2::new(rect.right() - 70.0, rect.center().y),
            Vec2::splat(28.0),
        );
        let download = icons::button_at(ui, download_rect, Icon::Download, Style::Ghost);

        if play.clicked() {
            RowAction::Play
        } else if download.clicked() {
            RowAction::Export
        } else if clicked {
            RowAction::Play
        } else {
            RowAction::None
        }
    }

    fn charts_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui);

        if self.charts.songs.is_empty() {
            empty_state(ui, Icon::Music, "暂无榜单数据", "稍后再来看看");
            return;
        }

        let songs = self.charts.songs.clone();
        let artists = self.charts.artists.clone();
        let mut search_for: Option<String> = None;

        ui.horizontal_top(|ui| {
            ui.add_space(GUTTER);
            let column_w = (ui.available_width() - GUTTER * 2.0 - 24.0) / 2.0;

            for (title, rows) in [("歌曲", &songs), ("歌手", &artists)] {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(
                            ui.cursor().min,
                            Vec2::new(column_w, ui.available_height()),
                        ))
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                child.label(
                    egui::RichText::new(title)
                        .size(10.0)
                        .color(TEXT_FAINT),
                );
                child.add_space(8.0);

                for (i, row) in rows.iter().enumerate() {
                    let (r, row_response) = child
                        .allocate_exact_size(Vec2::new(column_w, 34.0), Sense::click());
                    let painter = child.painter();

                    if row_response.hovered() {
                        icons::card(&painter, r, RADIUS_SM, CARD, None);
                    }

                    painter.text(
                        Pos2::new(r.left() + 16.0, r.center().y),
                        Align2::CENTER_CENTER,
                        format!("{}", i + 1),
                        FontId::proportional(T_FAINT),
                        if i < 3 { TEXT_MUTED } else { TEXT_FAINT },
                    );

                    painter.text(
                        Pos2::new(r.left() + 36.0, r.center().y),
                        Align2::LEFT_CENTER,
                        &row.name,
                        FontId::proportional(T_BODY),
                        TEXT,
                    );
                    let name_rect = Rect::from_min_max(
                        Pos2::new(r.left() + 36.0, r.center().y - 10.0),
                        Pos2::new(r.right() - 60.0, r.center().y + 10.0),
                    );
                    child.interact(
                        name_rect,
                        child.scope_id().with(("song", i, title)),
                        Sense::hover(),
                    )
                    .widget_info({
                        let n = row.name.clone();
                        move || egui::WidgetInfo::labeled(egui::Role::Label, true, n.clone())
                    });
                    painter.text(
                        Pos2::new(r.right() - 8.0, r.center().y),
                        Align2::RIGHT_CENTER,
                        format!("{}", row.plays),
                        FontId::proportional(T_FAINT),
                        TEXT_FAINT,
                    );

                    if row_response.clicked() {
                        search_for = Some(row.name.clone());
                    }
                }

                ui.add_space(column_w + 24.0);
            }
        });

        if let Some(name) = search_for {
            self.page = Page::Search;
            self.query = name.clone();
            self.last_query = name.clone();
            self.backend.send(Request::Search(name));
        }
    }

    fn library_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui);

        ui.horizontal(|ui| {
            ui.add_space(GUTTER);
            let count = self.library.len();
            ui.label(
                egui::RichText::new(format!("{count} 首 · {}", format_bytes(self.cache_bytes)))
                    .size(T_FAINT)
                    .color(TEXT_FAINT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(GUTTER);
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("清空缓存").size(T_META).color(DANGER),
                    ))
                    .clicked()
                {
                    self.backend.send(Request::ClearCache);
                }
                if ui
                    .add(egui::Button::new(egui::RichText::new("刷新").size(T_META)))
                    .clicked()
                {
                    self.backend.send(Request::Library);
                }
            });
        });
        ui.add_space(10.0);

        if self.library.is_empty() {
            empty_state(
                ui,
                Icon::Library,
                "还没有缓存的曲目",
                "在搜索页点「下载」或直接播放，曲目会留在本地",
            );
            return;
        }

        let mut to_play: Option<PathBuf> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let rows = self.library.clone();
                for (i, row) in rows.iter().enumerate() {
                    let (slot, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width() - GUTTER * 2.0, 56.0),
                        Sense::hover(),
                    );
                    let rect = Rect::from_min_max(
                        Pos2::new(slot.left() + GUTTER, slot.top()),
                        Pos2::new(slot.right() + GUTTER, slot.bottom()),
                    );

                    let play_rect = Rect::from_center_size(
                        Pos2::new(rect.right() - 24.0, rect.center().y),
                        Vec2::splat(28.0),
                    );
                    // Keep the play button out of the row's hover surface so a
                    // click on it cannot also start row playback twice.
                    let body = Rect::from_min_max(rect.min, Pos2::new(play_rect.left() - 12.0, rect.max.y));
                    let hover = ui
                        .interact(body, ui.scope_id().with(("lib", i)), Sense::click())
                        .hovered();

                    let painter = ui.painter().clone();
                    if hover {
                        icons::card(&painter, rect, RADIUS_SM, CARD, None);
                    }

                    icons::paint(
                        &painter,
                        Rect::from_center_size(
                            Pos2::new(rect.left() + 18.0, rect.center().y),
                            Vec2::splat(16.0),
                        ),
                        Icon::Music,
                        if hover { TEXT_MUTED } else { TEXT_FAINT },
                    );

                    painter.text(
                        Pos2::new(rect.left() + 40.0, rect.center().y - 9.0),
                        Align2::LEFT_CENTER,
                        &row.title,
                        FontId::proportional(T_BODY),
                        TEXT,
                    );
                    let title_rect = Rect::from_min_max(
                        Pos2::new(rect.left() + 40.0, rect.center().y - 18.0),
                        Pos2::new(rect.right() - 120.0, rect.center().y),
                    );
                    ui.interact(
                        title_rect,
                        ui.scope_id().with(("lib-title", i)),
                        Sense::hover(),
                    )
                    .widget_info({
                        let t = row.title.clone();
                        move || egui::WidgetInfo::labeled(egui::Role::Label, true, t.clone())
                    });
                    if !row.subtitle.is_empty() {
                        painter.text(
                            Pos2::new(rect.left() + 40.0, rect.center().y + 10.0),
                            Align2::LEFT_CENTER,
                            &row.subtitle,
                            FontId::proportional(T_FAINT),
                            TEXT_FAINT,
                        );
                    }

                    painter.text(
                        Pos2::new(play_rect.left() - 16.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        format_bytes(row.bytes),
                        FontId::proportional(T_FAINT),
                        TEXT_FAINT,
                    );

                    if icons::button_at(ui, play_rect, Icon::Play, Style::Ghost).clicked() {
                        to_play = Some(row.path.clone());
                    }

                    // Hairline between rows, inset to the text column.
                    if i + 1 < rows.len() {
                        let y = rect.bottom();
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                Pos2::new(rect.left() + 40.0, y),
                                Pos2::new(rect.right(), y + 1.0),
                            ),
                            0.0,
                            BORDER,
                        );
                    }
                }
                ui.add_space(GUTTER);
            });

        if let Some(path) = to_play {
            if let Some(engine) = &self.engine {
                engine.play(path);
            }
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui);

        let quality = self.quality;
        let mut picked: Option<Quality> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(GUTTER);
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - GUTTER).min(620.0));

                        // ---- quality
                        section(ui, "音质偏好", |ui| {
                            let tiers = [
                                (Quality::Master, "至臻母带", "24bit / 192kHz FLAC，含歌词"),
                                (Quality::Surround, "高清环绕声", "较小体积，不含歌词"),
                            ];
                            let rows: Vec<(&str, &str)> =
                                tiers.iter().map(|(_, t, d)| (*t, *d)).collect();
                            let current = tiers.iter().position(|(q, _, _)| *q == quality);

                            if let Some(index) = choice_group(ui, &rows, current) {
                                let tier = tiers[index].0;
                                let mut cfg =
                                    twentyfourbit_core::Config::load().unwrap_or_default();
                                cfg.quality = tier;
                                let _ = cfg.save();
                                picked = Some(tier);
                            }
                        });

                        // ---- storage
                        section(ui, "存储", |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "缓存 {} 个文件 · {}",
                                    self.cache_files,
                                    format_bytes(self.cache_bytes)
                                ))
                                .size(T_BODY)
                                .color(TEXT_MUTED),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    twentyfourbit_core::Cache::default_cache()
                                        .root()
                                        .display()
                                        .to_string(),
                                )
                                .size(T_FAINT)
                                .color(TEXT_FAINT),
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "下载目录 {}",
                                        self.export_dir.display()
                                    ))
                                    .size(T_FAINT)
                                    .color(TEXT_FAINT),
                                );
                            });
                            ui.add_space(12.0);
                            if ui
                                .add(egui::Button::new(
                                    egui::RichText::new("清空缓存").size(T_META).color(DANGER),
                                ))
                                .clicked()
                            {
                                self.backend.send(Request::ClearCache);
                            }
                        });

                        // ---- account
                        section(ui, "账号", |ui| {
                            ui.label(
                                egui::RichText::new(match &self.account {
                                    Some(a) => format!("已登录 · {a}"),
                                    None => "游客 · 每天可下载数首".to_owned(),
                                })
                                .size(T_BODY)
                                .color(TEXT_MUTED),
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.login_account)
                                        .hint_text("账号")
                                        .desired_width(150.0)
                                        .font(FontId::proportional(T_BODY)),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.login_password)
                                        .hint_text("密码")
                                        .password(true)
                                        .desired_width(150.0)
                                        .font(FontId::proportional(T_BODY)),
                                );
                                let ready = !self.login_account.is_empty()
                                    && !self.login_password.is_empty();
                                if ui
                                    .add_enabled(
                                        ready,
                                        egui::Button::new(
                                            egui::RichText::new("登录").size(T_META),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.backend.send(Request::Login {
                                        account: self.login_account.clone(),
                                        password: self.login_password.clone(),
                                    });
                                }
                            });
                        });

                        // ---- about
                        section(ui, "关于", |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "24bit {} · 非官方客户端 · MIT OR Apache-2.0",
                                    env!("CARGO_PKG_VERSION")
                                ))
                                .size(T_FAINT)
                                .color(TEXT_FAINT),
                            );
                            if let Some(font) = &self.cjk_font {
                                ui.label(
                                    egui::RichText::new(format!("界面字体 {font}"))
                                        .size(T_FAINT)
                                        .color(TEXT_FAINT),
                                );
                            }
                        });

                        ui.add_space(GUTTER);
                    });
                });
            });
    }


}

/// What a list row's click resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    None,
    Play,
    Export,
}

/// Announce hand-painted text to assistive technology.
///
/// Rows and headings are drawn with the painter so their geometry is exact, but
/// painted glyphs are invisible to a screen reader — and to the headless UI
/// tests, which query the same accessibility tree. This registers a label over
/// the painted area, so the visual and accessible trees stay in step.
fn expose(ui: &mut egui::Ui, rect: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let id = ui
        .scope_id()
        .with(("a11y", text, rect.min.x as i32, rect.min.y as i32));
    let response = ui.interact(rect, id, Sense::hover());
    let owned = text.to_owned();
    response.widget_info(move || {
        egui::WidgetInfo::labeled(egui::Role::Label, true, owned.clone())
    });
}

/// A rotating arc, drawn as a polyline.
fn spinner(painter: &egui::Painter, centre: Pos2, radius: f32, colour: Color32, time: f64) {
    let start = (time * 4.0) as f32 % std::f32::consts::TAU;
    let sweep = std::f32::consts::TAU * 0.75;
    let steps = 18;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + sweep * (i as f32 / steps as f32);
            Pos2::new(centre.x + a.cos() * radius, centre.y + a.sin() * radius)
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.6, colour)));
}

/// Where "download to disk" puts files by default.
fn default_export_dir() -> PathBuf {
    dirs::audio_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("24bit")
}

/// A grouped list of choices, drawn as one card with hairline dividers — the
/// shape macOS uses for inset lists. Returns the index the user picked.
fn choice_group(ui: &mut egui::Ui, rows: &[(&str, &str)], selected: Option<usize>) -> Option<usize> {
    let row_h = 52.0;
    let height = row_h * rows.len() as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    icons::card(ui.painter(), rect, RADIUS_SM, CARD, None);

    let mut picked = None;
    for (i, (title, detail)) in rows.iter().enumerate() {
        let top = rect.top() + row_h * i as f32;
        let row = egui::Rect::from_min_max(
            Pos2::new(rect.left(), top),
            Pos2::new(rect.right(), top + row_h),
        );
        let response = ui.interact(
            row,
            ui.scope_id().with(("choice", i)),
            Sense::click(),
        );

        let painter = ui.painter().clone();
        let is_selected = selected == Some(i);

        if response.hovered() {
            painter.rect_filled(row, 0.0, CARD_HOVER);
        }
        if is_selected {
            // A tint, not a border: the group already has one container.
            painter.rect_filled(row, 0.0, ACCENT_SOFT);
        }

        let centre = Pos2::new(row.left() + 22.0, row.center().y);
        painter.circle_stroke(centre, 7.0, Stroke::new(1.5, BORDER_STRONG));
        if is_selected {
            painter.circle_filled(centre, 3.5, TEXT);
        }

        painter.text(
            Pos2::new(row.left() + 42.0, row.center().y - 8.0),
            Align2::LEFT_CENTER,
            *title,
            FontId::proportional(T_BODY),
            TEXT,
        );
        painter.text(
            Pos2::new(row.left() + 42.0, row.center().y + 9.0),
            Align2::LEFT_CENTER,
            *detail,
            FontId::proportional(T_FAINT),
            TEXT_FAINT,
        );

        // Divider between rows, inset to the text column.
        if i + 1 < rows.len() {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    Pos2::new(row.left() + 42.0, row.bottom() - 1.0),
                    Pos2::new(row.right() - 16.0, row.bottom()),
                ),
                0.0,
                BORDER,
            );
        }

        if response.clicked() {
            picked = Some(i);
        }
    }

    picked
}

/// A centred placeholder: an icon, a line of what happened, and what to do.
fn empty_state(ui: &mut egui::Ui, icon: Icon, title: &str, hint: &str) {
    let height = ui.available_height().max(200.0);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter().clone();
    let centre = Pos2::new(rect.center().x, rect.top() + height * 0.34);

    painter.circle_filled(centre, 32.0, CARD);
    icons::paint(
        &painter,
        Rect::from_center_size(centre, Vec2::splat(26.0)),
        icon,
        TEXT_FAINT,
    );

    painter.text(
        Pos2::new(centre.x, centre.y + 52.0),
        Align2::CENTER_TOP,
        title,
        FontId::proportional(T_SECTION),
        TEXT_MUTED,
    );
    let title_rect = Rect::from_center_size(
        Pos2::new(centre.x, centre.y + 60.0),
        Vec2::new(ui.available_width().min(420.0), 18.0),
    );
    expose(ui, title_rect, title);
    painter.text(
        Pos2::new(centre.x, centre.y + 74.0),
        Align2::CENTER_TOP,
        hint,
        FontId::proportional(T_META),
        TEXT_FAINT,
    );
}

/// A titled group on a card surface.
fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.label(
        egui::RichText::new(title)
            .size(T_SECTION)
            .strong()
            .color(TEXT),
    );
    ui.add_space(GAP);
    egui::Frame::NONE
        .fill(CARD)
        .corner_radius(RADIUS as u8)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui);
        });
    ui.add_space(20.0);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Without an explicit clear colour the window shows the desktop through
        // any area a panel does not cover.
        [BG.r() as f32 / 255.0, BG.g() as f32 / 255.0, BG.b() as f32 / 255.0, 1.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(engine) = &self.engine {
            engine.shutdown();
        }
    }
}

impl App {
    /// Paint one frame. Split from [`eframe::App::ui`] so headless tests drive
    /// exactly the same code path.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.drain_events();
        self.covers.poll(&ctx);

        if self
            .engine
            .as_ref()
            .and_then(|e| e.snapshot().transport)
            .map(|t| t.phase() == Phase::Playing)
            .unwrap_or(false)
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // `screen_rect` rather than the root ui rect: with a seamless titlebar the
        // root ui starts below the window edge, which would leave a bare strip.
        paint_backdrop(ui.painter(), ctx.viewport_rect());

        self.header(ui);
        self.status_bar(ui);
        self.transport(ui);
        self.sidebar(ui);
        self.lyrics_panel(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| match self.page {
                Page::Search => self.search_page(ui),
                Page::Charts => self.charts_page(ui),
                Page::Library => self.library_page(ui),
                Page::Settings => self.settings_page(ui),
            });
    }
}

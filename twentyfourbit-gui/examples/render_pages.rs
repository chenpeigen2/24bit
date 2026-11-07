//! Render every page to a PNG, headlessly.
//!
//! Reviewing an interface needs a picture, and taking over someone's screen to
//! get one is both rude and unreliable. `egui_kittest` renders the real widget
//! tree off-screen, so the layout can be inspected and compared frame to frame.
//!
//!     cargo run -p twentyfourbit-gui --release --example render_pages -- /tmp/shots

use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use twentyfourbit_core::{ChartEntry, SearchItem, TrackDetail, WeeklyCharts};
use twentyfourbit_gui::app::{App, Page};
use twentyfourbit_gui::backend::LibraryRow;

const W: f32 = 1080.0;
const H: f32 = 720.0;

fn samples() -> Vec<SearchItem> {
    let rows = [
        ("富士山下", "陈奕迅", "What's Going On…?"),
        ("富士山下 (Live)", "陈奕迅", "DUO 陈奕迅2010演唱会"),
        ("爱情转移", "陈奕迅", "认了吧"),
        ("海阔天空", "Beyond", "华纳廿三周年纪念精选系列"),
        ("晴天", "周杰伦", "叶惠美"),
        ("青花瓷", "周杰伦", "我很忙"),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (name, player, album))| SearchItem {
            id: format!("id-{i}"),
            cover: String::new(),
            name: (*name).to_owned(),
            player: (*player).to_owned(),
            album: (*album).to_owned(),
        })
        .collect()
}

fn charts() -> WeeklyCharts {
    let songs = [
        "富士山下", "渡口", "海阔天空", "晴天", "季候风", "青花瓷", "夜曲", "如愿", "稻香", "红豆",
    ];
    let artists = [
        "周杰伦", "陈奕迅", "王菲", "Beyond", "张学友", "蔡琴", "邓紫棋", "王杰", "谭咏麟", "林俊杰",
    ];
    WeeklyCharts {
        songs: songs
            .iter()
            .enumerate()
            .map(|(i, n)| ChartEntry {
                name: (*n).to_owned(),
                plays: 1418 - (i as u64) * 97,
            })
            .collect(),
        artists: artists
            .iter()
            .enumerate()
            .map(|(i, n)| ChartEntry {
                name: (*n).to_owned(),
                plays: 4300 - (i as u64) * 310,
            })
            .collect(),
    }
}

fn library() -> Vec<LibraryRow> {
    (0..4)
        .map(|i| LibraryRow {
            name: format!("track-{i}.flac"),
            title: "富士山下".to_owned(),
            subtitle: format!("陈奕迅 · 至臻母带 · 第 {} 版", i + 1),
            path: PathBuf::from(format!("/tmp/track-{i}.flac")),
            bytes: 156_129_336,
        })
        .collect()
}

fn detail() -> TrackDetail {
    TrackDetail {
        id: "id-0".into(),
        cover: String::new(),
        name: "富士山下".into(),
        player: "陈奕迅".into(),
        album: "What's Going On…?".into(),
        download_url: String::new(),
        size: "148.90MB".into(),
        quality: "至臻母带".into(),
        format: "flac".into(),
        lrc: Some(
            "[00:00.000] 作词 : 林夕\n\
             [00:18.500] 拦路雨偏似雪花 饮泣的你冻吗\n\
             [00:25.000] 这风褛我给你磨到有襟花\n\
             [00:32.000] 连调了职也不怕 怎么始终牵挂\n\
             [00:39.000] 苦心选中今天想车你回家\n\
             [00:46.000] 原谅我不再送花 伤口应要结疤\n\
             [00:53.000] 花瓣铺满心里坟场才害怕"
                .into(),
        ),
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/shots".to_owned());
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap();

    std::env::set_var("TWENTYFOURBIT_BASE_URL", "http://127.0.0.1:1");

    // Each page gets its own process-wide harness; keeping them separate stops
    // one page's scroll position leaking into the next.
    for (name, page) in [
        ("01-search-empty", Page::Search),
        ("02-search-results", Page::Search),
        ("03-charts", Page::Charts),
        ("04-library", Page::Library),
        ("05-settings", Page::Settings),
    ] {
        let mut harness = Harness::builder().with_size(egui::vec2(W, H)).build_ui_state(
            |ui, state: &mut Option<App>| {
                let app = state.get_or_insert_with(|| {
                    let ctx = ui.ctx().clone();
                    App::with_context(&ctx)
                });
                app.draw(ui);
            },
            None,
        );

        {
            let app = harness.state_mut().as_mut().unwrap();
            app.set_page(page);

            if name == "02-search-results" {
                app.seed_results(samples());
            }
            if name == "03-charts" {
                app.seed_charts(charts());
            }
            if name == "04-library" {
                app.seed_library(library(), 156_129_336 * 4);
            }
        }

        if name == "02-search-results" {
            // Put the app into a playing state so the transport and lyrics show.
            let app = harness.state_mut().as_mut().unwrap();
            app.adopt_track(detail(), PathBuf::from("/tmp/nonexistent.flac"));
        }

        harness.run_steps(6);
        let image = harness.render().expect("render");
        let path = out.join(format!("{name}.png"));
        image.save(&path).unwrap();
        println!("{}", path.display());
    }
}

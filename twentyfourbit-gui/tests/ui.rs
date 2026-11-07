//! Headless UI tests.
//!
//! These drive the real widget tree through `egui_kittest`, which runs egui
//! without a window. They catch what the compiler cannot: a page that panics
//! while painting, a nav button that does not switch pages, or a transport bar
//! that claims to be playing when nothing is loaded.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use twentyfourbit_gui::app::{App, Page};

type State = Option<App>;

/// Build a harness around the real app.
///
/// The backend is pointed at a dead port so tests never touch the network; a
/// failed request surfaces as an error line, which is exactly what a user sees
/// when they are offline.
fn harness() -> Harness<'static, State> {
    std::env::set_var("TWENTYFOURBIT_BASE_URL", "http://127.0.0.1:1");
    Harness::builder().build_ui_state(
        |ui, state: &mut State| {
            let app = state.get_or_insert_with(|| {
                let ctx = ui.ctx().clone();
                App::with_context(&ctx)
            });
            app.draw(ui);
        },
        None,
    )
}

fn app<'a>(harness: &'a Harness<'_, State>) -> &'a App {
    harness.state().as_ref().expect("app was built on the first frame")
}

#[test]
fn the_window_paints_without_panicking() {
    let mut harness = harness();
    harness.run_steps(4);
}

#[test]
fn it_starts_on_the_search_page() {
    let mut harness = harness();
    harness.run_steps(4);
    assert_eq!(app(&harness).page(), Page::Search);
}

#[test]
fn an_empty_search_shows_a_hint_rather_than_nothing() {
    let mut harness = harness();
    harness.run_steps(4);
    // A blank pane would leave the user wondering whether the app is broken.
    assert!(
        harness
            .get_all_by_label_contains("输入关键词开始搜索")
            .count()
            > 0,
        "an empty search pane must say what to do"
    );
}

#[test]
fn the_sidebar_lists_every_page() {
    let mut harness = harness();
    harness.run_steps(4);
    for label in ["搜索", "榜单", "音乐库", "设置"] {
        assert!(
            harness.get_all_by_label(label).count() >= 1,
            "the sidebar should offer a {label:?} entry"
        );
    }
}

#[test]
fn clicking_a_nav_entry_switches_pages() {
    let mut harness = harness();
    harness.run_steps(4);

    harness.get_by_label("设置").click();
    harness.run_steps(4);
    assert_eq!(app(&harness).page(), Page::Settings);

    harness.get_by_label("音乐库").click();
    harness.run_steps(4);
    assert_eq!(app(&harness).page(), Page::Library);

    harness.get_by_label("榜单").click();
    harness.run_steps(4);
    assert_eq!(app(&harness).page(), Page::Charts);
}

#[test]
fn the_transport_bar_prompts_when_idle() {
    let mut harness = harness();
    harness.run_steps(4);
    // With nothing loaded there is nothing to seek or pause.
    assert!(
        harness
            .get_all_by_label_contains("选一首歌开始播放")
            .count()
            > 0,
        "the transport bar must say what to do when nothing is loaded"
    );
}

#[test]
fn the_settings_page_exposes_quality_and_cache() {
    let mut harness = harness();
    harness.run_steps(4);

    harness.get_by_label("设置").click();
    harness.run_steps(4);

    assert!(harness.get_all_by_label_contains("音质偏好").count() > 0);
    assert!(harness.get_all_by_label_contains("缓存").count() > 0);
    assert!(harness.get_all_by_label_contains("账号").count() > 0);
}

#[test]
fn the_app_survives_repeated_repaints() {
    let mut harness = harness();
    for _ in 0..5 {
        harness.run_steps(4);
    }
}

/// A frame must render to an actual image, not just "not panic". This catches
/// layout bugs that produce an empty or single-colour window.
///
/// Gated behind `render-check` because it needs a GPU adapter; the rest of the
/// suite runs on a machine with none.
#[cfg(feature = "render-check")]
#[test]
fn a_frame_renders_pixels() {
    let mut harness = harness();
    harness.run_steps(4);

    // CI runners have no GPU. Skip rather than fail: a red build for missing
    // hardware teaches people to ignore the build.
    let image = match harness.render() {
        Ok(image) => image,
        Err(e) => {
            eprintln!("skipping: no renderer available ({e})");
            return;
        }
    };
    let distinct: std::collections::HashSet<[u8; 4]> = image
        .pixels()
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect();

    assert!(
        distinct.len() > 8,
        "rendered frame has only {} distinct colours, which means nothing was drawn",
        distinct.len()
    );
}

// ---------------------------------------------------------------- lyrics

fn detail_with_lyrics() -> twentyfourbit_core::TrackDetail {
    twentyfourbit_core::TrackDetail {
        id: "a1".into(),
        cover: String::new(),
        name: "富士山下".into(),
        player: "陈奕迅".into(),
        album: "What's Going On…?".into(),
        download_url: String::new(),
        size: "148.90MB".into(),
        quality: "至臻母带".into(),
        format: "flac".into(),
        lrc: Some(
            "[00:00.000] 作词 : 林夕\n[00:18.500] 拦路雨偏似雪花\n[00:25.000] 饮泣的你冻吗"
                .into(),
        ),
    }
}

#[test]
fn loading_a_track_with_lyrics_parses_them() {
    let mut harness = harness();
    harness.run_steps(4);

    {
        let state = harness.state_mut();
        let app = state.as_mut().unwrap();
        app.adopt_track(detail_with_lyrics(), std::path::PathBuf::from("/nonexistent.flac"));
    }
    harness.run_steps(4);

    let app = app(&harness);
    assert_eq!(app.lyrics().lines.len(), 3);
    assert_eq!(app.now_playing_title(), Some("富士山下"));
}

#[test]
fn lyrics_appear_in_the_panel() {
    let mut harness = harness();
    harness.run_steps(4);

    {
        let state = harness.state_mut();
        let app = state.as_mut().unwrap();
        app.adopt_track(detail_with_lyrics(), std::path::PathBuf::from("/nonexistent.flac"));
    }
    harness.run_steps(6);

    assert!(
        harness.get_all_by_label_contains("拦路雨偏似雪花").count() > 0,
        "the lyrics panel must show the loaded lines"
    );
    assert!(harness.get_all_by_label_contains("歌词").count() > 0);
}

#[test]
fn a_track_without_lyrics_does_not_open_the_panel() {
    let mut harness = harness();
    harness.run_steps(4);

    let mut detail = detail_with_lyrics();
    detail.lrc = None;
    {
        let state = harness.state_mut();
        state
            .as_mut()
            .unwrap()
            .adopt_track(detail, std::path::PathBuf::from("/nonexistent.flac"));
    }
    harness.run_steps(6);

    let app = app(&harness);
    assert!(app.lyrics().is_empty());
    assert_eq!(
        harness
            .query_all_by_label_contains("拦路雨偏似雪花")
            .count(),
        0,
        "no lyrics means no lyric rows"
    );
}

#[test]
fn the_transport_bar_shows_the_loaded_title() {
    let mut harness = harness();
    harness.run_steps(4);

    {
        let state = harness.state_mut();
        state
            .as_mut()
            .unwrap()
            .adopt_track(detail_with_lyrics(), std::path::PathBuf::from("/nonexistent.flac"));
    }
    harness.run_steps(6);

    assert!(harness.get_all_by_label_contains("富士山下").count() > 0);
}

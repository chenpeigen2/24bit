//! Behavioural tests for the LRC lyrics parser.
//!
//! The master quality tier ships lyrics with every track, so they are already
//! in hand — this parser is what turns them into something scrollable.
//!
//! LRC is a timestamped line format and its edge cases are all real: multiple
//! timestamps can share one line (the chorus), sub-second precision varies by
//! three orders of magnitude between files, and an `[offset:]` tag shifts
//! everything.

use std::time::Duration;
use twentyfourbit_gui::lyrics::Lyrics;

#[test]
fn parses_timestamps_and_text() {
    let lrc = "[00:12.34] 第一行\n[00:15.00] 第二行";
    let lyrics = Lyrics::parse(lrc);

    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[0].at, Duration::from_millis(12_340));
    assert_eq!(lyrics.lines[0].text, "第一行");
    assert_eq!(lyrics.lines[1].at, Duration::from_secs(15));
}

/// The site's own payload uses this exact shape (captured from `findMusicPage`).
#[test]
fn parses_a_captured_site_payload() {
    let lrc = "[00:00.000] 作词 : 林夕\n\
               [00:00.675] 作曲 : Christopher Chak\n\
               [00:01.350] 编曲 : 陈珀/C. Y. Kong\n\
               [00:18.500] 拦路雨偏似雪花";
    let lyrics = Lyrics::parse(lrc);

    assert_eq!(lyrics.lines.len(), 4);
    assert_eq!(lyrics.lines[0].text, "作词 : 林夕");
    assert_eq!(lyrics.lines[0].at, Duration::ZERO);
    assert_eq!(lyrics.lines[3].at, Duration::from_millis(18_500));
}

/// A chorus is written once with several timestamps on the same line.
#[test]
fn one_line_with_several_timestamps_becomes_several_entries() {
    let lrc = "[00:10.00][01:20.00][02:30.00] 副歌";
    let lyrics = Lyrics::parse(lrc);

    assert_eq!(lyrics.lines.len(), 3);
    assert!(lyrics.lines.iter().all(|l| l.text == "副歌"));
    assert_eq!(lyrics.lines[0].at, Duration::from_secs(10));
    assert_eq!(lyrics.lines[1].at, Duration::from_secs(80));
    assert_eq!(lyrics.lines[2].at, Duration::from_secs(150));
}

/// Metadata tags are not lyrics and must not appear in the scroll.
#[test]
fn metadata_tags_are_ignored() {
    let lrc = "[ti:富士山下]\n[ar:陈奕迅]\n[al:What's Going On]\n[by:someone]\n\
               [00:05.00] 真正的歌词";
    let lyrics = Lyrics::parse(lrc);

    assert_eq!(lyrics.lines.len(), 1);
    assert_eq!(lyrics.lines[0].text, "真正的歌词");
}

/// Files in the wild drop the fractional part entirely.
#[test]
fn second_precision_without_milliseconds_is_accepted() {
    let lyrics = Lyrics::parse("[01:05] 整秒");
    assert_eq!(lyrics.lines[0].at, Duration::from_secs(65));
}

/// One and two digit fractions are tenths and hundredths, not milliseconds.
#[test]
fn fractional_digits_scale_to_milliseconds() {
    assert_eq!(
        Lyrics::parse("[00:01.5] x").lines[0].at,
        Duration::from_millis(1_500)
    );
    assert_eq!(
        Lyrics::parse("[00:01.50] x").lines[0].at,
        Duration::from_millis(1_500)
    );
    assert_eq!(
        Lyrics::parse("[00:01.500] x").lines[0].at,
        Duration::from_millis(1_500)
    );
}

/// A positive offset means the lyrics are early and must be delayed; a negative
/// one pulls them forward.
#[test]
fn offset_shifts_every_timestamp() {
    let lrc = "[offset:+500]\n[00:10.00] 行";
    let lyrics = Lyrics::parse(lrc);
    assert_eq!(lyrics.lines[0].at, Duration::from_millis(10_500));

    let lrc = "[offset:-500]\n[00:10.00] 行";
    let lyrics = Lyrics::parse(lrc);
    assert_eq!(lyrics.lines[0].at, Duration::from_millis(9_500));
}

/// A negative offset larger than the first timestamp must not underflow.
#[test]
fn offset_cannot_push_a_line_before_zero() {
    let lyrics = Lyrics::parse("[offset:-5000]\n[00:01.00] 行");
    assert_eq!(lyrics.lines[0].at, Duration::ZERO);
}

/// Lines out of order (common in hand-edited files) are sorted so that
/// `index_at` can binary search.
#[test]
fn lines_are_sorted_by_time() {
    let lrc = "[00:30.00] 第三\n[00:10.00] 第一\n[00:20.00] 第二";
    let lyrics = Lyrics::parse(lrc);

    let times: Vec<u64> = lyrics
        .lines
        .iter()
        .map(|l| l.at.as_millis() as u64)
        .collect();
    assert_eq!(times, vec![10_000, 20_000, 30_000]);
}

#[test]
fn index_at_reports_the_line_being_sung() {
    let lyrics = Lyrics::parse("[00:01.00] a\n[00:02.00] b\n[00:03.00] c");

    assert_eq!(lyrics.index_at(Duration::ZERO), None, "before the first line");
    assert_eq!(lyrics.index_at(Duration::from_millis(1_500)), Some(0));
    assert_eq!(lyrics.index_at(Duration::from_secs(2)), Some(1));
    assert_eq!(lyrics.index_at(Duration::from_secs(99)), Some(2), "past the end");
}

/// Blank lines and junk must be skipped rather than producing empty entries.
#[test]
fn blank_and_malformed_lines_are_skipped() {
    let lrc = "[00:01.00] a\n\n   \nnot a lyric line\n[00:02.00] b";
    let lyrics = Lyrics::parse(lrc);

    assert_eq!(lyrics.lines.len(), 2);
    assert!(lyrics.lines.iter().all(|l| !l.text.is_empty()));
}

/// An empty or absent payload is a normal state: the surround tier has no
/// lyrics at all.
#[test]
fn empty_input_yields_no_lines() {
    assert!(Lyrics::parse("").is_empty());
    assert!(Lyrics::parse("   \n\n").is_empty());
    assert_eq!(Lyrics::parse("").index_at(Duration::from_secs(1)), None);
}

/// Trailing whitespace in the text is trimmed but internal spacing is kept.
#[test]
fn text_is_trimmed_but_not_collapsed() {
    let lyrics = Lyrics::parse("[00:01.00]  a  b  \n");
    assert_eq!(lyrics.lines[0].text, "a  b");
}

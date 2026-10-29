use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::Theme;

fn render(banner: &Banner, width: u16) -> (String, BannerHitMap) {
    let area = Rect::new(0, 2, width, 1);
    let mut buf = Buffer::empty(Rect::new(0, 0, width, 4));
    let hits = banner.render(area, &mut buf, &Theme::default());
    let line = (0..width)
        .map(|x| buf[(x, 2)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned();
    (line, hits)
}

fn banner() -> Banner {
    Banner::new("Merge in progress · 2 conflicted files")
}

#[test]
fn shows_the_message_the_buttons_and_their_keys() {
    let (line, _) = render(&banner(), 90);
    assert_eq!(
        line,
        " Merge in progress · 2 conflicted files   [Continue] [Abort]   C continue  A abort"
    );
}

#[test]
fn clicks_resolve_to_the_button_drawn_there() {
    let (line, hits) = render(&banner(), 90);
    // Every character on the line is one column wide.
    let column = |text: &str| {
        let byte = line.find(text).unwrap();
        u16::try_from(line[..byte].chars().count()).unwrap()
    };
    let continue_x = column("[Continue]");
    let abort_x = column("[Abort]");
    assert_eq!(hits.hit(continue_x, 2), Some(BannerButton::Continue));
    assert_eq!(hits.hit(continue_x + 9, 2), Some(BannerButton::Continue));
    assert_eq!(hits.hit(abort_x, 2), Some(BannerButton::Abort));
    assert_eq!(hits.hit(abort_x + 6, 2), Some(BannerButton::Abort));
    assert_eq!(
        hits.hit(abort_x - 1, 2),
        None,
        "the gap between the buttons"
    );
    assert_eq!(hits.hit(continue_x, 3), None, "another row");
    assert_eq!(hits.hit(2, 2), None, "the message");
}

#[test]
fn narrow_terminals_drop_the_key_hints_then_cut_the_message() {
    let (line, hits) = render(&banner(), 64);
    assert_eq!(
        line,
        " Merge in progress · 2 conflicted files   [Continue] [Abort]"
    );
    assert!(hits.button(BannerButton::Abort).is_some());

    let (line, hits) = render(&banner(), 30);
    assert!(line.ends_with("[Continue] [Abort]"), "{line}");
    assert!(line.starts_with(" Merge"), "{line}");
    let abort = hits.button(BannerButton::Abort).unwrap();
    assert!(abort.right() <= 30, "{abort:?}");
}

#[test]
fn an_empty_area_draws_nothing() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    let hits = banner().render(Rect::new(0, 0, 0, 0), &mut buf, &Theme::default());
    assert_eq!(hits, BannerHitMap::default());
}

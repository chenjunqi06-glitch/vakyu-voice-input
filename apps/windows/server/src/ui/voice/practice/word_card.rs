//! 跟读界面内的离线词卡：只画当前结果，不管理录音或跟读进度。

use super::fit_status;
use crate::ui::candidates::{theme::Theme, view};
use crate::ui::voice::{fill_round, rgb};
use windows::Win32::Foundation::RECT;
pub(super) fn word_card_bounds(width: i32, dpi: u32) -> (RECT, RECT, RECT) {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let padding = scale(22);
    let card_width = scale(260).min((width - padding * 2).max(scale(120)));
    let card = RECT {
        left: width - padding - card_width,
        top: scale(44),
        right: width - padding,
        bottom: scale(184),
    };
    let button_top = card.bottom - scale(31);
    let button_bottom = card.bottom - scale(8);
    let play = RECT {
        left: card.left + scale(8),
        top: button_top,
        right: card.left + scale(104),
        bottom: button_bottom,
    };
    let close = RECT {
        left: card.right - scale(68),
        top: button_top,
        right: card.right - scale(8),
        bottom: button_bottom,
    };
    (card, play, close)
}

pub(super) fn contains(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

pub(super) fn paint_word_card(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    theme: &Theme,
    width: i32,
    _height: i32,
    dpi: u32,
    card: &crate::dispatch::PracticeWordCard,
    learn_english: bool,
) {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let (bounds, play, close) = word_card_bounds(width, dpi);
    fill_round(hdc, bounds, rgb(0x4c, 0x37, 0x68), scale(11));
    let text_width = bounds.right - bounds.left - scale(20);
    let title = fit_status(&card.word, text_width, |text| {
        view::measure(hdc, theme.text_font, text).cx
    });
    let meaning = fit_status(
        &card.meaning.as_deref().map_or_else(
            || {
                if learn_english {
                    "释义：未收录"
                } else {
                    "Meaning: no offline entry"
                }
                .to_owned()
            },
            |meaning| {
                format!(
                    "{}: {meaning}",
                    if learn_english { "释义" } else { "Meaning" }
                )
            },
        ),
        text_width,
        |text| view::measure(hdc, theme.annotation_font, text).cx,
    );
    let pronunciation = fit_status(
        &card.pronunciation.as_deref().map_or_else(
            || {
                if learn_english {
                    "音标：未收录"
                } else {
                    "Pronunciation: unavailable"
                }
                .to_owned()
            },
            |pronunciation| {
                format!(
                    "{}: {pronunciation}",
                    if learn_english {
                        "音标"
                    } else if card
                        .word
                        .chars()
                        .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
                    {
                        "Pinyin"
                    } else {
                        "IPA"
                    }
                )
            },
        ),
        text_width,
        |text| view::measure(hdc, theme.annotation_font, text).cx,
    );
    let hint = card.hint.as_deref().unwrap_or("");
    let hint = fit_status(hint, text_width, |text| {
        view::measure(hdc, theme.annotation_font, text).cx
    });
    view::draw_text(
        hdc,
        theme.text_font,
        rgb(0xff, 0xff, 0xff),
        bounds.left + scale(10),
        bounds.top + scale(7),
        &title,
    );
    view::draw_text(
        hdc,
        theme.annotation_font,
        rgb(0xe5, 0xd8, 0xf4),
        bounds.left + scale(10),
        bounds.top + scale(31),
        &meaning,
    );
    view::draw_text(
        hdc,
        theme.annotation_font,
        rgb(0xe5, 0xd8, 0xf4),
        bounds.left + scale(10),
        bounds.top + scale(50),
        &pronunciation,
    );
    if !hint.is_empty() {
        view::draw_text(
            hdc,
            theme.annotation_font,
            rgb(0xc3, 0xab, 0xe3),
            bounds.left + scale(10),
            bounds.top + scale(69),
            &hint,
        );
    }
    for (button, label) in [
        (
            play,
            if learn_english {
                "播放发音"
            } else {
                "Listen"
            },
        ),
        (close, if learn_english { "关闭" } else { "Close" }),
    ] {
        fill_round(hdc, button, rgb(0x70, 0x4f, 0x9a), scale(6));
        let text = view::measure(hdc, theme.annotation_font, label);
        view::draw_text(
            hdc,
            theme.annotation_font,
            rgb(0xff, 0xff, 0xff),
            button.left + (button.right - button.left - text.cx) / 2,
            button.top + (button.bottom - button.top - text.cy) / 2,
            label,
        );
    }
}

//! 英语跟读浮窗：本机练习等级、中文原句、逐词着色与示范或录音状态。

mod layout;
mod run;
mod word_card;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC, SetBkMode, TRANSPARENT};
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNA, SetTimer, ShowWindow};
use windows::core::Result;

use crate::dispatch::{PracticeAction, PracticeView};
use crate::ui::candidates::theme::Theme;
use crate::ui::candidates::view;
use crate::ui::layered::{self, Layered};
use crate::ui::monitor;
use crate::ui::voice::frame::VoiceFrame;

use self::layout::{recognition_feedback, visible_range, wrap_sized_words};
use self::word_card::{contains, paint_word_card, word_card_bounds};
use super::{
    GLOBAL_HELD, HELD_TIMER, PRACTICE_VISIBLE, VoiceWindow, anchor_point, fill_round, paint_meter,
    place, rgb, transcript_lines,
};

impl VoiceWindow {
    pub(in crate::ui) fn show_practice(&self, practice: &PracticeView, anchor: RECT) {
        // 允许同步目标检查期间过期的 true 租约；显式 false 仍拒绝迟到的跟读帧。
        if GLOBAL_HELD.get().is_some_and(|(held, _)| !held) {
            self.hide();
            return;
        }
        PRACTICE_VISIBLE.set(true);
        self.set_transparent(false);
        self.events.practice_view.replace(Some(practice.clone()));
        self.cache_frame(
            VoiceFrame::Practice {
                view: practice.clone(),
                anchor,
            },
            practice.recording,
        );
        // 识别临时文本更新频繁，但逐词高亮只应在已读词数变化时启动过渡。
        if self.last_practice_matched.get() != Some(practice.matched_words) {
            self.last_practice_matched.set(Some(practice.matched_words));
            self.reset_word_transition();
        }
        self.sync_theme(anchor);

        // 识别器的 interim 更新可能每 10~20ms 到达；分层窗口合成和逐字测量都在 UI
        // 线程上完成，超过 30fps 只会增加 CPU 占用。缓存的最新 PracticeView 会在
        // 下一次音量刷新或下一次识别更新时绘出，因此不会丢失最终文字。
        let now = Instant::now();
        if practice.recording
            && self.last_practice_render.get().is_some_and(|previous| {
                now.saturating_duration_since(previous) < Duration::from_millis(33)
            })
        {
            return;
        }
        if let Err(error) = self.render_practice(practice, anchor) {
            tracing::warn!(%error, "绘制英语跟读浮窗失败");
            self.hide();
            return;
        }
        self.last_practice_render.set(Some(now));
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        unsafe { SetTimer(Some(self.hwnd), HELD_TIMER, 60, None) };
    }

    pub(super) fn render_practice(&self, practice: &PracticeView, anchor: RECT) -> Result<()> {
        let dpi = self.dpi.get();
        let scale = |px: i32| (px * dpi as i32 / 96).max(1);
        let theme = self.theme.borrow();
        let margin = layered::shadow_margin(dpi);
        let work = monitor::work_area_near(anchor_point(anchor));
        let width = scale(540).min((work.right - work.left - 2 * margin).max(scale(220)));
        let padding = scale(22);
        let inner_width = width - padding * 2;
        let hdc = unsafe { GetDC(Some(self.hwnd)) };
        let chinese = if practice.learn_english {
            transcript_lines(hdc, &theme, &practice.original, inner_width)
        } else {
            layout::english_source_lines(&practice.original, inner_width, |text| {
                view::measure(hdc, theme.text_font, text).cx
            })
        };
        let source_line_height = view::measure(hdc, theme.text_font, "中文")
            .cy
            .max(scale(18))
            + scale(7);
        let reading_height = if practice.learn_english { 0 } else { scale(20) };
        let line_height = source_line_height + reading_height;
        let english_space = view::measure(hdc, theme.text_font, " ").cx;
        let reading_widths: Vec<_> = practice
            .pronunciations
            .iter()
            .map(|reading| view::measure(hdc, theme.annotation_font, reading).cx)
            .collect();
        let english = wrap_sized_words(
            &practice.words,
            inner_width,
            english_space,
            |index, text| {
                view::measure(hdc, theme.text_font, text)
                    .cx
                    .max(reading_widths.get(index).copied().unwrap_or(0))
            },
        );
        let matched = practice.matched_words.min(practice.words.len());
        let mut visible = visible_range(&english, matched);
        if !practice.learn_english && visible.len() > 3 {
            let current = english
                .iter()
                .position(|line| line.iter().any(|word| word.index >= matched))
                .unwrap_or_else(|| english.len().saturating_sub(1));
            visible.start = current
                .saturating_sub(1)
                .min(english.len().saturating_sub(3));
            visible.end = (visible.start + 3).min(english.len());
        }
        let english_count = visible.len().max(1);
        let status_offset = scale(if practice.recording { 34 } else { 17 });
        let status = fit_status(&practice.status, inner_width - status_offset, |text| {
            view::measure(hdc, theme.annotation_font, text).cx
        });
        let recognized_feedback = recognition_feedback(
            &practice.recognized,
            practice.recording,
            practice.learn_english,
            inner_width,
            |text| view::measure(hdc, theme.annotation_font, text).cx,
        );
        let feedback = fit_status(&practice.feedback, inner_width, |text| {
            view::measure(hdc, theme.annotation_font, text).cx
        });
        let level_label = fit_status(&practice.level_label, inner_width / 2, |text| {
            view::measure(hdc, theme.annotation_font, text).cx
        });
        let level_width = view::measure(hdc, theme.annotation_font, &level_label).cx + scale(22);
        let level_progress = fit_status(
            &practice.level_progress,
            inner_width - level_width - scale(14),
            |text| view::measure(hdc, theme.annotation_font, text).cx,
        );
        let word_hint = match practice.words.get(matched) {
            Some(word) => format!(
                "{}  {word}",
                if practice.learn_english {
                    "当前词"
                } else {
                    "Current"
                }
            ),
            None if practice.words.is_empty() => String::new(),
            None => if practice.learn_english {
                "本句已读完"
            } else {
                "Reading complete"
            }
            .to_owned(),
        };
        let word_hint = fit_status(&word_hint, inner_width * 3 / 5, |text| {
            view::measure(hdc, theme.annotation_font, text).cx
        });
        unsafe { ReleaseDC(Some(self.hwnd), hdc) };
        let chinese_top = scale(112);
        let divider_top = chinese_top + chinese.len() as i32 * source_line_height + scale(7);
        let english_top = divider_top + scale(36);
        let progress_top = english_top + english_count as i32 * line_height + scale(14);
        let footer_height = if practice.actions_enabled {
            164
        } else if !feedback.is_empty() {
            116
        } else if recognized_feedback.is_empty() {
            72
        } else {
            96
        };
        let content = (width, progress_top + scale(footer_height));
        let mut word_hits = Vec::new();
        for (row, line) in english[visible.clone()].iter().enumerate() {
            let mut x = padding;
            let y = english_top + row as i32 * line_height;
            for word in line {
                word_hits.push((
                    RECT {
                        left: x,
                        top: y,
                        right: x + word.width,
                        bottom: y + line_height,
                    },
                    word.index,
                ));
                x += word.width + english_space;
            }
        }
        self.events.practice_word_hits.replace(word_hits);
        self.events.practice_content.set(content);
        self.events.practice_dpi.set(dpi);
        let position = place(work, content, margin, scale(16));
        let accent = rgb(0xff, 0xff, 0xff);
        let badge_background = rgb(0x80, 0x55, 0xb7);
        let muted = rgb(0xc3, 0xab, 0xe3);
        let rule = rgb(0x91, 0x72, 0xb2);
        layered::composite(
            self.hwnd,
            &Layered {
                content,
                margin,
                win_pos: (position.0 - margin, position.1 - margin),
                win_size: (content.0 + margin * 2, content.1 + margin * 2),
                background: theme.background,
                background_end: theme.background_end,
                corner_radius: scale(18),
                paint: &|hdc, _| {
                    unsafe { SetBkMode(hdc, TRANSPARENT) };
                    view::draw_text(
                        hdc,
                        theme.text_font,
                        accent,
                        padding,
                        scale(19),
                        if practice.learn_english {
                            "英语跟读"
                        } else {
                            "中文跟读 · Learn Chinese"
                        },
                    );
                    let hint = if practice.learn_english {
                        "Esc 取消"
                    } else {
                        "Esc Cancel"
                    };
                    let hint_width = view::measure(hdc, theme.annotation_font, hint).cx;
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        width - padding - hint_width,
                        scale(22),
                        hint,
                    );
                    fill_round(
                        hdc,
                        RECT {
                            left: padding,
                            top: scale(50),
                            right: padding + level_width,
                            bottom: scale(77),
                        },
                        badge_background,
                        scale(13),
                    );
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        accent,
                        padding + scale(11),
                        scale(56),
                        &level_label,
                    );
                    let level_progress_width =
                        view::measure(hdc, theme.annotation_font, &level_progress).cx;
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        width - padding - level_progress_width,
                        scale(56),
                        &level_progress,
                    );
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        padding,
                        scale(90),
                        if practice.learn_english {
                            "中文原句 · 读完英文后填入"
                        } else {
                            "Original English · Inserted after reading Chinese"
                        },
                    );
                    for (index, line) in chinese.iter().enumerate() {
                        view::draw_text(
                            hdc,
                            theme.text_font,
                            theme.text_color,
                            padding,
                            chinese_top + index as i32 * source_line_height,
                            line,
                        );
                    }
                    fill_round(
                        hdc,
                        RECT {
                            left: padding,
                            top: divider_top,
                            right: width - padding,
                            bottom: divider_top + scale(1),
                        },
                        rule,
                        1,
                    );
                    let range_hint = if !practice.learn_english {
                        "中文译句 · Chinese + Pinyin"
                    } else if visible.start > 0 || visible.end < english.len() {
                        "英文译句 · 随跟读滚动"
                    } else {
                        "英文译句"
                    };
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        padding,
                        divider_top + scale(13),
                        range_hint,
                    );
                    let word_hint_width = view::measure(hdc, theme.annotation_font, &word_hint).cx;
                    let range_hint_width = view::measure(hdc, theme.annotation_font, range_hint).cx;
                    if word_hint_width + range_hint_width + scale(12) <= inner_width {
                        view::draw_text(
                            hdc,
                            theme.annotation_font,
                            accent,
                            width - padding - word_hint_width,
                            divider_top + scale(13),
                            &word_hint,
                        );
                    }
                    if english.is_empty() {
                        view::draw_text(
                            hdc,
                            theme.text_font,
                            muted,
                            padding,
                            english_top,
                            if practice.learn_english {
                                "正在准备英文译句…"
                            } else {
                                "Preparing Chinese translation…"
                            },
                        );
                    }
                    let space = view::measure(hdc, theme.text_font, " ").cx;
                    for (row, line) in english[visible.clone()].iter().enumerate() {
                        let mut x = padding;
                        let y = english_top + row as i32 * line_height;
                        for word in line {
                            let color = if word.index < matched {
                                blend_color(
                                    muted,
                                    theme.text_color,
                                    self.word_transition_progress(),
                                )
                            } else {
                                muted
                            };
                            if !practice.learn_english {
                                let reading = practice
                                    .pronunciations
                                    .get(word.index)
                                    .map_or("", String::as_str);
                                let reading_width =
                                    reading_widths.get(word.index).copied().unwrap_or(0);
                                view::draw_text(
                                    hdc,
                                    theme.annotation_font,
                                    color,
                                    x + (word.width - reading_width) / 2,
                                    y,
                                    reading,
                                );
                            }
                            let text_width = view::measure(hdc, theme.text_font, &word.text).cx;
                            view::draw_text(
                                hdc,
                                theme.text_font,
                                color,
                                x + (word.width - text_width) / 2,
                                y + reading_height,
                                &word.text,
                            );
                            if word.index == matched {
                                fill_round(
                                    hdc,
                                    RECT {
                                        left: x,
                                        top: y + line_height - scale(3),
                                        right: x + word.width,
                                        bottom: y + line_height - scale(1),
                                    },
                                    accent,
                                    scale(1),
                                );
                            }
                            x += word.width + space;
                        }
                    }
                    fill_round(
                        hdc,
                        RECT {
                            left: padding,
                            top: progress_top,
                            right: width - padding,
                            bottom: progress_top + scale(4),
                        },
                        rule,
                        scale(2),
                    );
                    if matched > 0 && !practice.words.is_empty() {
                        let filled = (inner_width as usize * matched / practice.words.len()) as i32;
                        fill_round(
                            hdc,
                            RECT {
                                left: padding,
                                top: progress_top,
                                right: padding + filled.max(scale(4)),
                                bottom: progress_top + scale(4),
                            },
                            accent,
                            scale(2),
                        );
                    }
                    let progress = if practice.learn_english {
                        format!("已读 {matched} / {} 词", practice.words.len())
                    } else {
                        format!("Read {matched} / {}", practice.words.len())
                    };
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        accent,
                        padding,
                        progress_top + scale(15),
                        &progress,
                    );
                    let completion_hint = if practice.learn_english {
                        "读完后填入中文"
                    } else {
                        "Complete to insert English"
                    };
                    let hint_width = view::measure(hdc, theme.annotation_font, completion_hint).cx;
                    let progress_width = view::measure(hdc, theme.annotation_font, &progress).cx;
                    if hint_width + progress_width + scale(16) <= inner_width {
                        view::draw_text(
                            hdc,
                            theme.annotation_font,
                            theme.gloss_color,
                            width - padding - hint_width,
                            progress_top + scale(15),
                            completion_hint,
                        );
                    }
                    if practice.recording {
                        paint_meter(
                            hdc,
                            dpi,
                            padding,
                            progress_top + scale(48),
                            &self.meter.borrow(),
                        );
                    } else {
                        fill_round(
                            hdc,
                            RECT {
                                left: padding,
                                top: progress_top + scale(45),
                                right: padding + scale(6),
                                bottom: progress_top + scale(51),
                            },
                            accent,
                            scale(3),
                        );
                    }
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        padding + status_offset,
                        progress_top + scale(40),
                        &status,
                    );
                    if !recognized_feedback.is_empty() {
                        let color = if practice.recognized.trim().is_empty() {
                            theme.gloss_color
                        } else {
                            theme.text_color
                        };
                        view::draw_text(
                            hdc,
                            theme.annotation_font,
                            color,
                            padding,
                            progress_top + scale(66),
                            &recognized_feedback,
                        );
                    }
                    if !feedback.is_empty() {
                        view::draw_text(
                            hdc,
                            theme.annotation_font,
                            theme.text_color,
                            padding,
                            progress_top + scale(88),
                            &feedback,
                        );
                    }
                    if practice.actions_enabled {
                        paint_actions(hdc, &theme, width, content.1, dpi, practice.learn_english);
                    }
                    if let Some(card) = &practice.word_card {
                        paint_word_card(
                            hdc,
                            &theme,
                            width,
                            content.1,
                            dpi,
                            card,
                            practice.learn_english,
                        );
                    }
                },
            },
        )
    }
}

/// 坐标相对卡片内容区，不包含阴影外边距。
pub(super) fn action_at(
    practice: &PracticeView,
    x: i32,
    y: i32,
    content: (i32, i32),
    dpi: u32,
) -> Option<PracticeAction> {
    if practice.word_card.is_some() {
        let (card, play, close) = word_card_bounds(content.0, dpi);
        if contains(play, x, y) {
            return Some(PracticeAction::SpeakWord);
        }
        if contains(close, x, y) {
            return Some(PracticeAction::CloseWordCard);
        }
        if contains(card, x, y) {
            return None;
        }
    }
    if !practice.actions_enabled {
        return None;
    }
    [
        PracticeAction::Retry,
        PracticeAction::SkipCurrent,
        PracticeAction::AcceptCurrent,
        PracticeAction::Restart,
    ]
    .into_iter()
    .zip(action_bounds(content.0, content.1, dpi))
    .find_map(|(action, bounds)| contains(bounds, x, y).then_some(action))
}

pub(super) fn word_at(
    practice: &PracticeView,
    x: i32,
    y: i32,
    hits: &[(RECT, usize)],
) -> Option<PracticeAction> {
    if practice.word_card.is_some() {
        return None;
    }
    hits.iter()
        .find(|(bounds, _)| contains(*bounds, x, y))
        .map(|(_, index)| PracticeAction::LookupWord(*index))
}

/// 绘制和命中共用布局，避免窄窗口或 DPI 缩放后点到相邻操作。
fn action_bounds(width: i32, height: i32, dpi: u32) -> [RECT; 4] {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let padding = scale(22);
    let gap = scale(8);
    let top = height - scale(44);
    let available = (width - padding * 2 - gap * 3).max(0);
    std::array::from_fn(|index| {
        let index = index as i32;
        RECT {
            left: padding + available * index / 4 + gap * index,
            top,
            right: padding + available * (index + 1) / 4 + gap * index,
            bottom: top + scale(30),
        }
    })
}

fn paint_actions(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    theme: &Theme,
    width: i32,
    height: i32,
    dpi: u32,
    learn_english: bool,
) {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let labels = if learn_english {
        ["继续补读", "跳过此词", "我读对了", "整句重读"]
    } else {
        ["Continue", "Skip", "Accept", "Restart"]
    };
    for (label, bounds) in labels.into_iter().zip(action_bounds(width, height, dpi)) {
        fill_round(hdc, bounds, rgb(0x63, 0x42, 0x91), scale(7));
        let button_width = bounds.right - bounds.left;
        let text_width = view::measure(hdc, theme.annotation_font, label).cx;
        let text_y = bounds.top
            + (bounds.bottom - bounds.top - view::measure(hdc, theme.annotation_font, label).cy)
                / 2;
        view::draw_text(
            hdc,
            theme.annotation_font,
            rgb(0xff, 0xff, 0xff),
            bounds.left + (button_width - text_width) / 2,
            text_y,
            label,
        );
    }
}

fn blend_color(
    from: windows::Win32::Foundation::COLORREF,
    to: windows::Win32::Foundation::COLORREF,
    amount: f32,
) -> windows::Win32::Foundation::COLORREF {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let a = ((from.0 >> shift) & 0xff) as f32;
        let b = ((to.0 >> shift) & 0xff) as f32;
        (a + (b - a) * amount).round() as u32
    };
    windows::Win32::Foundation::COLORREF(channel(0) | (channel(8) << 8) | (channel(16) << 16))
}

fn fit_status(text: &str, width: i32, measure: impl Fn(&str) -> i32) -> String {
    if measure(text) <= width {
        return text.to_owned();
    }
    let mut fitted = String::new();
    for ch in text.chars().take(120) {
        fitted.push(ch);
        if measure(&format!("{fitted}…")) > width {
            fitted.pop();
            break;
        }
    }
    fitted.push('…');
    fitted
}

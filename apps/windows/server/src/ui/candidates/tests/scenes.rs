//! 候选窗基准的固定虚构中英文场景。

use qingjian_core::{Candidate, CandidateKind, CandidateList, Language, Sense, Translation};
use qingjian_platform::protocol::{Frame, PreeditKind, PreeditSegment};
use qingjian_platform::{LayoutMode, PreeditMode, ThemeMode};
pub(super) fn frame(long: bool, english: bool, revision: usize) -> Frame {
    let chinese = if long {
        "今天下午我们计划先去图书馆借书然后一起到公园散步"
    } else {
        "图书馆"
    };
    let english_text = if long {
        "We plan to visit the library this afternoon and then take a walk in the park."
    } else {
        "library"
    };
    let (text, gloss, language, kind) = if english {
        (
            english_text,
            chinese,
            Language::Chinese,
            CandidateKind::English,
        )
    } else {
        (
            chinese,
            english_text,
            Language::English,
            CandidateKind::Chinese,
        )
    };
    let count = if long { 9 } else { 5 };
    let candidates = (0..count)
        .map(|index| Candidate {
            text: if index == 0 {
                text.to_owned()
            } else {
                format!("{text} {}", index + 1)
            },
            kind,
            syllables: Vec::new(),
            reading: None,
            translation: Some(Translation::new(
                language,
                vec![Sense {
                    part_of_speech: None,
                    text: gloss.to_owned(),
                    reading: None,
                    fresh: index == revision % count,
                }],
            )),
            aux_code: None,
        })
        .collect();
    let preedit = if long {
        "jin'tian'xia'wu'wo'men'ji'hua'xian'qu'tu'shu'guan'jie'shu'ran'hou'yi'qi'dao'gong'yuan'san'bu"
    } else {
        "tu'shu'guan"
    };
    let preedit = format!("{preedit}{}", "n".repeat(revision % 4));
    Frame {
        cursor: preedit.chars().count(),
        preedit: vec![PreeditSegment {
            text: preedit,
            kind: PreeditKind::Typed,
        }],
        preedit_mode: PreeditMode::Both,
        candidates: CandidateList { items: candidates },
        highlight: revision % count,
        page: 0,
        page_count: 4,
        layout: if long {
            LayoutMode::Vertical
        } else {
            LayoutMode::Horizontal
        },
        theme: ThemeMode::System,
        ..Frame::default()
    }
}

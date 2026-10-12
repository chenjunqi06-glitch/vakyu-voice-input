//! 固定候选内容与逐字增长的 ASCII 拼音展示帧；不涉及引擎查询或词库。

use qingjian_render::{Frame, Preedit, Row, Tone};

pub(super) fn fixed() -> Frame {
    let rows = [
        ("我今天要去吃饭", "I am going to eat today."),
        ("我今天", "today I"),
        ("我", "I; me"),
        ("今天", "today"),
        ("要去", "going to"),
        ("吃饭", "have a meal"),
        ("明天下午", "tomorrow afternoon"),
        ("三点", "three o'clock"),
        ("快递", "express delivery"),
    ]
    .iter()
    .enumerate()
    .map(|(index, (text, annotation))| {
        let mut row = Row::plain(index, *text);
        row.annotation.push(((*annotation).to_owned(), Tone::Gloss));
        row
    })
    .collect();
    Frame {
        preedit: Some(Preedit::plain("wo'jin'tian'yao'qu'chi'fan", 26)),
        rows,
        highlighted: Some(0),
        footer: Some("1/6".to_owned()),
        ..Frame::default()
    }
}

pub(super) fn growing(keys: usize) -> Vec<Frame> {
    const PINYIN: &str = "wojintianyaoquchifanmingtianxiawusandianqunakuaidiranhouhuijiayongdiannaolianxiyingyufayin";
    let input: String = PINYIN.chars().cycle().take(keys).collect();
    (1..=keys)
        .map(|length| {
            let mut frame = fixed();
            frame.preedit = Some(Preedit::plain(&input[..length], length));
            frame.highlighted = Some(length % frame.rows.len());
            frame
        })
        .collect()
}

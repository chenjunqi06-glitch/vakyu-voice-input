//! 数量与逐位数字的保守等值；保留前导零、小数尾零，不猜大数或电话号码读法。

pub(super) fn take(characters: &[char], start: usize) -> Option<(usize, String)> {
    if !is_number(*characters.get(start)?) {
        return None;
    }
    let mut end = start;
    while let Some(character) = characters.get(end) {
        if is_number(*character) {
            end += 1;
        } else if character.is_whitespace() {
            let next = next_non_space(characters, end);
            if characters
                .get(next)
                .is_some_and(|character| is_number(*character))
            {
                end = next;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    let integer_characters: Vec<char> = characters[start..end]
        .iter()
        .copied()
        .filter(|character| !character.is_whitespace())
        .collect();
    let integer = canonical_integer(&integer_characters)?;
    let mut canonical = integer;
    let point = next_non_space(characters, end);
    let fractional = next_non_space(characters, point.saturating_add(1));
    if characters
        .get(point)
        .is_some_and(|character| matches!(character, '.' | '点' | '點'))
        && characters
            .get(fractional)
            .and_then(|character| digit(*character))
            .is_some()
    {
        canonical.push('.');
        end = fractional;
        while let Some(value) = characters.get(end).and_then(|character| digit(*character)) {
            canonical.push(char::from(b'0' + value));
            end += 1;
            let next = next_non_space(characters, end);
            if characters
                .get(next)
                .and_then(|character| digit(*character))
                .is_some()
            {
                end = next;
            }
        }
    }
    Some((end - start, canonical))
}

fn next_non_space(characters: &[char], mut index: usize) -> usize {
    while characters
        .get(index)
        .is_some_and(|character| character.is_whitespace())
    {
        index += 1;
    }
    index
}

fn canonical_integer(characters: &[char]) -> Option<String> {
    if characters
        .iter()
        .all(|character| digit(*character).is_some())
    {
        return Some(
            characters
                .iter()
                .map(|character| char::from(b'0' + digit(*character).unwrap()))
                .collect(),
        );
    }
    let mut total = 0_u32;
    let mut pending = None;
    let mut previous_scale = 10_000;
    for (index, character) in characters.iter().copied().enumerate() {
        if let Some(value) = digit(character) {
            if value == 0 && pending.is_none() {
                continue;
            }
            if pending.replace(u32::from(value)).is_some() {
                return None;
            }
        } else {
            let scale = match character {
                '十' => 10,
                '百' => 100,
                '千' => 1000,
                _ => return None,
            };
            if scale >= previous_scale {
                return None;
            }
            let multiplier = pending
                .take()
                .or_else(|| (index == 0 && scale == 10).then_some(1))?;
            if multiplier == 0 {
                return None;
            }
            total += multiplier * scale;
            previous_scale = scale;
        }
    }
    // “一百二”口语可表示120，不猜省略的数量单位。
    if pending.is_some()
        && previous_scale > 10
        && characters.len() >= 2
        && characters[characters.len() - 2] != '零'
    {
        return None;
    }
    total += pending.unwrap_or(0);
    (total <= 9999).then(|| total.to_string())
}

fn is_number(character: char) -> bool {
    digit(character).is_some() || matches!(character, '十' | '百' | '千')
}

fn digit(character: char) -> Option<u8> {
    match character {
        '0' | '零' | '〇' => Some(0),
        '1' | '一' => Some(1),
        '2' | '二' | '两' | '兩' => Some(2),
        '3' | '三' => Some(3),
        '4' | '四' => Some(4),
        '5' | '五' => Some(5),
        '6' | '六' => Some(6),
        '7' | '七' => Some(7),
        '8' | '八' => Some(8),
        '9' | '九' => Some(9),
        _ => None,
    }
}

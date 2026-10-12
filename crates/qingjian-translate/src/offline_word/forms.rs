//! 保守的英文词形归一；必须命中本地词条才返回词义，并标明使用了原形。

pub(super) fn normalize(word: &str) -> String {
    word.trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .replace(['’', '‘'], "'")
        .to_ascii_lowercase()
}

pub(super) fn base_forms(word: &str) -> Vec<String> {
    let mut forms = Vec::new();
    if let Some(base) = irregular(word) {
        forms.push(base.to_owned());
    }
    if let Some(base) = word.strip_suffix("'s") {
        forms.push(base.to_owned());
    }
    if let Some(stem) = word.strip_suffix("ies").filter(|stem| stem.len() >= 2) {
        forms.push(format!("{stem}y"));
    }
    if let Some(stem) = word.strip_suffix("ied").filter(|stem| stem.len() >= 2) {
        forms.push(format!("{stem}y"));
    }
    for suffix in ["ing", "ed"] {
        if let Some(stem) = word.strip_suffix(suffix).filter(|stem| stem.len() >= 3) {
            if doubled_consonant(stem) {
                forms.push(stem[..stem.len() - 1].to_owned());
            }
            forms.push(format!("{stem}e"));
            forms.push(stem.to_owned());
        }
    }
    if let Some(stem) = word.strip_suffix("es").filter(|stem| stem.len() >= 2)
        && ["s", "x", "z", "ch", "sh"]
            .iter()
            .any(|ending| stem.ends_with(ending))
    {
        forms.push(stem.to_owned());
    }
    if let Some(stem) = word.strip_suffix('s').filter(|stem| stem.len() >= 3)
        && !word.ends_with("ss")
    {
        forms.push(stem.to_owned());
    }
    let mut unique = Vec::new();
    for form in forms {
        if !unique.contains(&form) {
            unique.push(form);
        }
    }
    // 只做明确的屈折变化，不把任意 -ly/-er 词切成可能完全不相关的词。
    unique
}

fn doubled_consonant(stem: &str) -> bool {
    let bytes = stem.as_bytes();
    bytes.len() >= 2
        && bytes[bytes.len() - 1] == bytes[bytes.len() - 2]
        && bytes[bytes.len() - 1].is_ascii_alphabetic()
        && !b"aeiou".contains(&bytes[bytes.len() - 1])
}

fn irregular(word: &str) -> Option<&'static str> {
    match word {
        "am" | "is" | "are" | "was" | "were" | "been" => Some("be"),
        "has" | "had" => Some("have"),
        "did" | "done" => Some("do"),
        "went" | "gone" => Some("go"),
        "bought" => Some("buy"),
        "brought" => Some("bring"),
        "thought" => Some("think"),
        "taught" => Some("teach"),
        "caught" => Some("catch"),
        "said" => Some("say"),
        "took" | "taken" => Some("take"),
        "gave" | "given" => Some("give"),
        "saw" | "seen" => Some("see"),
        "ate" | "eaten" => Some("eat"),
        "ran" => Some("run"),
        "wrote" | "written" => Some("write"),
        "spoke" | "spoken" => Some("speak"),
        "children" => Some("child"),
        "men" => Some("man"),
        "women" => Some("woman"),
        "mice" => Some("mouse"),
        "geese" => Some("goose"),
        "feet" => Some("foot"),
        "teeth" => Some("tooth"),
        _ => None,
    }
}

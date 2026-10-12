//! 省略锚点与受保护词；读音替代独立于省略预算，数字和否定仍须精确读到。

use super::latin::supported_text;

pub(super) fn protected(word: &str) -> bool {
    numeric(word) || matches!(word, "not" | "no" | "never" | "neither" | "nor" | "without")
}

pub(super) fn weak(word: &str) -> bool {
    matches!(word, "a" | "an" | "the" | "to" | "of")
}

pub(super) fn content(word: &str) -> bool {
    !weak(word)
        && !numeric(word)
        && !matches!(
            word,
            "i" | "you"
                | "he"
                | "she"
                | "it"
                | "we"
                | "they"
                | "me"
                | "him"
                | "her"
                | "us"
                | "them"
                | "my"
                | "your"
                | "his"
                | "its"
                | "our"
                | "their"
                | "am"
                | "is"
                | "are"
                | "was"
                | "were"
                | "be"
                | "been"
                | "being"
                | "do"
                | "does"
                | "did"
                | "have"
                | "has"
                | "had"
                | "will"
                | "would"
                | "shall"
                | "should"
                | "can"
                | "could"
                | "may"
                | "might"
                | "must"
                | "and"
                | "or"
                | "but"
                | "if"
                | "as"
                | "in"
                | "on"
                | "at"
                | "by"
                | "for"
                | "from"
                | "with"
                | "about"
                | "this"
                | "that"
                | "these"
                | "those"
                | "not"
                | "no"
                | "never"
                | "neither"
                | "nor"
                | "without"
        )
}

pub(super) fn filler(word: &str) -> bool {
    matches!(word, "um" | "uh" | "erm" | "er" | "hmm" | "ah")
}

pub(super) fn foreign_script_noise(word: &str) -> bool {
    !supported_text(word)
}

pub(super) fn numeric(word: &str) -> bool {
    matches!(
        word,
        "zero"
            | "one"
            | "two"
            | "three"
            | "four"
            | "five"
            | "six"
            | "seven"
            | "eight"
            | "nine"
            | "ten"
            | "eleven"
            | "twelve"
            | "thirteen"
            | "fourteen"
            | "fifteen"
            | "sixteen"
            | "seventeen"
            | "eighteen"
            | "nineteen"
            | "twenty"
            | "thirty"
            | "forty"
            | "fifty"
            | "sixty"
            | "seventy"
            | "eighty"
            | "ninety"
            | "hundred"
            | "thousand"
            | "million"
            | "billion"
            | "point"
            | "minus"
            | "plus"
            | "percent"
            | "first"
            | "second"
            | "third"
            | "fourth"
            | "fifth"
            | "sixth"
            | "seventh"
            | "eighth"
            | "ninth"
            | "tenth"
    )
}

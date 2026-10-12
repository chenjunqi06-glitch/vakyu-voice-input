//! 常见十进制数字转英文读法；保留前导零与小数尾零，避免把不同数字混为一谈。

const SMALL: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

pub(super) fn expand_number(word: &str, output: &mut Vec<String>) -> bool {
    if let Some((integer, decimal)) = word.split_once('.') {
        if digits(integer) && digits(decimal) {
            expand_integer(integer, output);
            output.push("point".to_owned());
            expand_digits(decimal, output);
            return true;
        }
    } else if digits(word) {
        expand_integer(word, output);
        return true;
    }
    false
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn expand_integer(text: &str, output: &mut Vec<String>) {
    if text.len() > 9 || (text.len() > 1 && text.starts_with('0')) {
        expand_digits(text, output);
    } else if let Ok(number) = text.parse::<u32>() {
        cardinal(number, output);
    }
}

fn expand_digits(text: &str, output: &mut Vec<String>) {
    output.extend(
        text.bytes()
            .map(|byte| SMALL[usize::from(byte - b'0')].to_owned()),
    );
}

fn cardinal(number: u32, output: &mut Vec<String>) {
    for (divisor, scale) in [
        (1_000_000, "million"),
        (1_000, "thousand"),
        (100, "hundred"),
    ] {
        if number >= divisor {
            cardinal(number / divisor, output);
            output.push(scale.to_owned());
            if !number.is_multiple_of(divisor) {
                cardinal(number % divisor, output);
            }
            return;
        }
    }
    if number < 20 {
        output.push(SMALL[number as usize].to_owned());
    } else {
        output.push(TENS[(number / 10) as usize].to_owned());
        if !number.is_multiple_of(10) {
            output.push(SMALL[(number % 10) as usize].to_owned());
        }
    }
}

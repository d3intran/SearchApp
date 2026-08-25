use regex::Regex;
use std::sync::LazyLock;

static STD_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z]+[/]?[A-Za-z]*\s*[0-9]").unwrap());

static STD_EXTRACT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]?\s*([0-9]{4})?",
    )
    .unwrap()
});

#[must_use]
pub fn normalize(code: &str) -> String {
    code.to_lowercase()
        .replace([' ', '/'], "")
        .replace(['\u{FF0D}', '\u{2014}'], "-")
        .replace('\u{FF1A}', ":")
}

#[must_use]
pub fn contains_code(input: &str) -> bool {
    STD_PREFIX_RE.is_match(input)
}

#[must_use]
pub fn extract_code(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(cap) = STD_EXTRACT_RE.captures(trimmed) {
        let prefix = cap[1].replace(' ', "");
        let number = cap[2].replace(' ', "");
        let year = cap.get(3).map_or("", |m| m.as_str());
        if year.is_empty() {
            format!("{} {}", prefix, number)
        } else {
            format!("{} {}-{}", prefix, number, year)
        }
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("GB/T 1234.1-2020"), "gbt1234.1-2020");
        assert_eq!(normalize("GY/T 222—2007"), "gyt222-2007");
    }

    #[test]
    fn test_contains_code() {
        assert!(contains_code("GB 1234-2020"));
        assert!(contains_code("GY/T 222-2007"));
        assert!(!contains_code("纯中文标准名称"));
    }

    #[test]
    fn test_extract_code() {
        assert_eq!(extract_code("GB/T 1234.1-2020 某某标准"), "GB/T 1234.1-2020");
        assert_eq!(extract_code("GB/T 1234.1"), "GB/T 1234.1");
    }
}

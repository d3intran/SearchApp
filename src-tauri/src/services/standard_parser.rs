pub fn normalize(code: &str) -> String {
    code.to_lowercase()
        .replace(' ', "")
        .replace('\u{FF0D}', "-")
        .replace('\u{FF1A}', ":")
        .replace('\u{2014}', "-")
}

pub fn extract_code(input: &str) -> String {
    let re = regex::Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]?\s*([0-9]{4})?"
    ).unwrap();

    let trimmed = input.trim();
    if let Some(cap) = re.captures(trimmed) {
        let prefix = cap[1].replace(' ', "");
        let number = cap[2].replace(' ', "");
        let year = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        if year.is_empty() {
            format!("{} {}", prefix, number)
        } else {
            format!("{} {}-{}", prefix, number, year)
        }
    } else {
        trimmed.to_string()
    }
}

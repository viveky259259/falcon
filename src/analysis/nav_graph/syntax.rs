use regex::Regex;

pub(super) fn call_bodies<'a>(source: &'a str, name: &str) -> Vec<(usize, &'a str)> {
    let mut bodies = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find(name) {
        let start = cursor + relative;
        let after_name = start + name.len();
        if !is_token_boundary(source, start, after_name) {
            cursor = after_name;
            continue;
        }
        let Some(open_relative) = source[after_name..].find('(') else {
            break;
        };
        let open = after_name + open_relative;
        if !source[after_name..open].trim().is_empty() {
            cursor = after_name;
            continue;
        }
        if let Some(close) = matching_paren(source, open) {
            bodies.push((start, &source[open + 1..close]));
            cursor = close + 1;
        } else {
            break;
        }
    }
    bodies
}

pub(super) fn capture(regex: &Regex, source: &str) -> Option<String> {
    regex
        .captures(source)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

pub(super) fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

pub(super) fn screen_id(widget: &str) -> String {
    format!("scr:{widget}")
}

pub(super) fn route_widget_name(route: &str) -> String {
    let mut result = String::new();
    for part in route.split(|character: char| !character.is_ascii_alphanumeric()) {
        if part.is_empty() {
            continue;
        }
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            result.extend(first.to_uppercase());
            result.extend(characters);
        }
    }
    if result.is_empty() {
        "UnknownRoute".to_string()
    } else {
        format!("{result}Page")
    }
}

fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate().skip(open) {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == active_quote {
                quote = None;
            }
            continue;
        }
        match *byte {
            b'\'' | b'"' => quote = Some(*byte),
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_token_boundary(source: &str, start: usize, end: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[end..].chars().next();
    !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
}

fn is_identifier_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

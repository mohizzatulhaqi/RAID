use serde_json::Value;

use super::NormalizedEvent;

const TOKEN_PREFIXES: [&str; 10] = [
    "sk-",
    "sk_live_",
    "sk_test_",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "AKIA",
    "AIza",
];
const SECRET_KEYS: [&str; 7] = [
    "password", "passwd", "secret", "token", "apikey", "api_key", "auth",
];
const MIN_TOKEN_LEN: usize = 16;
pub const REDACTED: &str = "[redacted]";

fn looks_like_token(word: &str) -> bool {
    word.len() >= MIN_TOKEN_LEN && TOKEN_PREFIXES.iter().any(|p| word.starts_with(p))
}

fn redact_word(word: &str, after_bearer: bool) -> String {
    let core = word.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | ',' | ';'));
    if core.is_empty() {
        return word.to_string();
    }
    if after_bearer || looks_like_token(core) {
        return word.replacen(core, REDACTED, 1);
    }
    if let Some(pos) = core.find(['=', ':']) {
        let (key, value) = core.split_at(pos);
        let value = &value[1..];
        let key_lower = key.to_lowercase();
        if !value.is_empty() && SECRET_KEYS.iter().any(|k| key_lower.contains(k)) {
            return word.replacen(core, &format!("{key}{}{REDACTED}", &core[pos..=pos]), 1);
        }
    }
    word.to_string()
}

/// Masks common credentials (API keys, tokens, `password=...`, bearer values).
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let mut after_bearer = false;
    let flush = |word: &mut String, out: &mut String, after_bearer: &mut bool| {
        if !word.is_empty() {
            out.push_str(&redact_word(word, *after_bearer));
            *after_bearer = word.eq_ignore_ascii_case("bearer");
            word.clear();
        }
    };
    for ch in text.chars() {
        if ch.is_whitespace() {
            flush(&mut word, &mut out, &mut after_bearer);
            out.push(ch);
        } else {
            word.push(ch);
        }
    }
    flush(&mut word, &mut out, &mut after_bearer);
    out
}

fn redact_value(value: &mut Value) {
    match value {
        Value::String(s) => *s = redact(s),
        Value::Array(items) => items.iter_mut().for_each(redact_value),
        Value::Object(map) => map.values_mut().for_each(redact_value),
        _ => {}
    }
}

/// Redacts every text field of an event before it is shown or stored.
pub fn redact_event(event: NormalizedEvent) -> NormalizedEvent {
    let Ok(mut value) = serde_json::to_value(&event) else {
        return event;
    };
    if let Some(payload) = value.get_mut("payload") {
        redact_value(payload);
    }
    serde_json::from_value(value).unwrap_or(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_token_shapes_are_masked() {
        assert_eq!(
            redact("export KEY=1 sk-ant-api03-abcdefghijklmnop done"),
            "export KEY=1 [redacted] done"
        );
        assert_eq!(redact("\"ghp_abcdefghijklmnop1234\""), "\"[redacted]\"");
    }

    #[test]
    fn secret_assignments_keep_the_key_and_hide_the_value() {
        assert_eq!(
            redact("DB_PASSWORD=hunter2 npm start"),
            "DB_PASSWORD=[redacted] npm start"
        );
        assert_eq!(redact("api_key: abc"), "api_key: abc");
        assert_eq!(redact("api_key:abc"), "api_key:[redacted]");
    }

    #[test]
    fn bearer_values_are_masked() {
        assert_eq!(
            redact("curl -H 'Authorization: Bearer abc.def' x"),
            "curl -H 'Authorization: Bearer [redacted]' x"
        );
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text = "npm run build && cargo test -- --nocapture\n  src/auth.rs";
        assert_eq!(redact(text), text);
        assert_eq!(redact("sk-short"), "sk-short");
    }
}

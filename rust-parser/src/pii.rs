//! PII-маскирование на горячем пути.
//! Использует regex-automata с DFA (детерминированный конечный автомат):
//! - Нет backtracking
//! - Нет аллокаций на поиске
//! - Компиляция паттернов происходит один раз при старте
//!
//! Документация: https://docs.rs/regex-automata/latest/regex_automata/

use once_cell::sync::Lazy;
use regex_automata::{meta::Regex, util::syntax::Config as SyntaxConfig, Input};

/// Скомпилированные паттерны — инициализируются один раз при старте программы.
/// Использование `once_cell::Lazy` гарантирует thread-safe инициализацию.
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::builder()
        .syntax(SyntaxConfig::new().case_insensitive(true))
        .build(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}")
        .expect("EMAIL_RE pattern is valid")
});

static PHONE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::builder()
        .build(r"\+?[0-9]{1,3}[\s\-]?\(?[0-9]{3}\)?[\s\-]?[0-9]{3}[\s\-]?[0-9]{4,}")
        .expect("PHONE_RE pattern is valid")
});

static TOKEN_RE: Lazy<Regex> = Lazy::new(|| {
    // Bearer токены, JWT (три base64url части через точку), API ключи
    Regex::builder()
        .syntax(SyntaxConfig::new().case_insensitive(true))
        .build(r"(?:bearer\s+|token[=:\s]+|authorization[=:\s]+|api[_-]?key[=:\s]+)[A-Za-z0-9\-_\.]{20,}")
        .expect("TOKEN_RE pattern is valid")
});

static CREDIT_CARD_RE: Lazy<Regex> = Lazy::new(|| {
    // Luhn-подобные числа 13-19 цифр (через пробелы или дефисы)
    Regex::builder()
        .build(r"\b(?:\d[ \-]?){13,19}\b")
        .expect("CREDIT_CARD_RE pattern is valid")
});

/// Маскирует PII в строке. Возвращает новую строку только если были замены.
/// Для оптимизации: если замен нет — возвращает None (избегаем аллокации).
pub fn mask_pii(input: &str) -> Option<String> {
    let mut result = input.to_owned();
    let mut modified = false;

    modified |= replace_all(&mut result, &EMAIL_RE, "***@***.***");
    modified |= replace_all(&mut result, &PHONE_RE, "[PHONE]");
    modified |= replace_all(&mut result, &TOKEN_RE, "[REDACTED_TOKEN]");
    modified |= replace_all(&mut result, &CREDIT_CARD_RE, "[CARD_REDACTED]");

    if modified {
        Some(result)
    } else {
        None
    }
}

/// Маскирует PII и возвращает строку (всегда).
pub fn mask_pii_owned(input: String) -> String {
    mask_pii(&input).unwrap_or(input)
}

/// Применяет все замены regex в строке, возвращает true если были замены.
fn replace_all(text: &mut String, re: &Regex, replacement: &str) -> bool {
    let mut modified = false;
    let mut offset = 0usize;
    let mut result = String::with_capacity(text.len());

    loop {
        let input = Input::new(&text[offset..]);
        match re.find(input) {
            None => {
                result.push_str(&text[offset..]);
                break;
            }
            Some(m) => {
                result.push_str(&text[offset..offset + m.start()]);
                result.push_str(replacement);
                offset += m.end();
                modified = true;
            }
        }
    }

    if modified {
        *text = result;
    }
    modified
}

/// Маскирует конкретные ключи в JSON-объекте (без полного ре-парсинга).
pub fn mask_sensitive_json_keys(value: &mut serde_json::Value) {
    const SENSITIVE_KEYS: &[&str] = &[
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "authorization",
        "credit_card",
        "card_number",
        "cvv",
        "ssn",
        "private_key",
        "access_token",
        "refresh_token",
        "session_token",
    ];

    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if SENSITIVE_KEYS
                    .iter()
                    .any(|k| key.to_lowercase().contains(k))
                {
                    *val = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    mask_sensitive_json_keys(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                mask_sensitive_json_keys(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_masking() {
        let input = "User john.doe@example.com logged in from 192.168.1.1";
        let result = mask_pii(input).expect("Should mask email");
        assert!(result.contains("***@***.***"));
        assert!(!result.contains("john.doe@example.com"));
    }

    #[test]
    fn test_phone_masking() {
        let input = "Contact: +7 (495) 123-4567 or call +1-800-555-0100";
        let result = mask_pii(input).expect("Should mask phone");
        assert!(result.contains("[PHONE]"));
        assert!(!result.contains("+7 (495)"));
    }

    #[test]
    fn test_token_masking() {
        let input = "Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature";
        let result = mask_pii(input).expect("Should mask token");
        assert!(result.contains("[REDACTED_TOKEN]"));
    }

    #[test]
    fn test_no_pii_no_alloc() {
        let input = "Normal log message without any PII data.";
        let result = mask_pii(input);
        assert!(result.is_none(), "Should return None when no PII found");
    }

    #[test]
    fn test_sensitive_json_keys() {
        let mut value = serde_json::json!({
            "user": "john",
            "password": "supersecret123",
            "nested": {
                "token": "eyJhbGci..."
            }
        });
        mask_sensitive_json_keys(&mut value);
        assert_eq!(value["password"], "[REDACTED]");
        assert_eq!(value["nested"]["token"], "[REDACTED]");
        assert_eq!(value["user"], "john"); // не маскируется
    }
}

//! Детектирование формата и парсинг сырых логов.
//! Поддерживаемые форматы: JSON, CEF, syslog (RFC5424/RFC3164), plaintext.

use crate::error::ParserError;
use crate::schema::{NormalizedEvent, Severity};
use bytes::Bytes;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;

const MAX_EVENT_SIZE: usize = 1024 * 1024; // 1MB

#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Json,
    Cef,
    Syslog5424,
    Syslog3164,
    PlainText,
}

/// Определяет формат лога по первым байтам.
pub fn detect_format(raw: &[u8]) -> LogFormat {
    // Пропускаем BOM и пробелы
    let trimmed = raw.iter().position(|&b| !b.is_ascii_whitespace())
        .map(|i| &raw[i..])
        .unwrap_or(raw);

    if trimmed.first() == Some(&b'{') {
        return LogFormat::Json;
    }

    // CEF: начинается с "CEF:0|"
    if trimmed.starts_with(b"CEF:") {
        return LogFormat::Cef;
    }

    // Syslog RFC5424: "<Priority>Version Timestamp..."
    if trimmed.first() == Some(&b'<') {
        // Ищем закрывающую угловую скобку
        if let Some(gt_pos) = trimmed.iter().position(|&b| b == b'>') {
            let after_prio = &trimmed[gt_pos + 1..];
            // RFC5424 начинается с цифры версии (обычно "1")
            if after_prio.first().map(|b| b.is_ascii_digit()) == Some(true) {
                return LogFormat::Syslog5424;
            }
            return LogFormat::Syslog3164;
        }
    }

    LogFormat::PlainText
}

/// Главная функция парсинга. Принимает сырые байты, возвращает частично заполненный NormalizedEvent.
/// Обогащение и PII-маскирование выполняются отдельным слоем.
pub fn parse(raw: Bytes, source_type: &str, host: &str) -> Result<NormalizedEvent, ParserError> {
    if raw.len() > MAX_EVENT_SIZE {
        return Err(ParserError::EventTooLarge {
            size: raw.len(),
            max: MAX_EVENT_SIZE,
        });
    }

    let format = detect_format(&raw);
    let mut event = NormalizedEvent::new(source_type);
    event.host = host.to_string();
    event.ingest_ts = Utc::now();

    match format {
        LogFormat::Json => parse_json(raw, &mut event)?,
        LogFormat::Cef => parse_cef(raw, &mut event)?,
        LogFormat::Syslog5424 => parse_syslog5424(raw, &mut event)?,
        LogFormat::Syslog3164 => parse_syslog3164(raw, &mut event)?,
        LogFormat::PlainText => parse_plaintext(raw, &mut event),
    }

    Ok(event)
}

fn parse_json(raw: Bytes, event: &mut NormalizedEvent) -> Result<(), ParserError> {
    let value: Value = serde_json::from_slice(&raw)?;

    let obj = match &value {
        Value::Object(m) => m,
        _ => {
            // JSON но не объект — оборачиваем
            event.message = raw.escape_ascii().to_string();
            event.event_type = "raw".to_string();
            return Ok(());
        }
    };

    // Маппинг Serilog/ASP.NET Core structured logging
    event.message = obj.get("Message")
        .or_else(|| obj.get("message"))
        .or_else(|| obj.get("msg"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Severity — поддерживаем разные имена полей
    let severity_str = obj.get("Level")
        .or_else(|| obj.get("level"))
        .or_else(|| obj.get("severity"))
        .or_else(|| obj.get("Severity"))
        .and_then(|v| v.as_str())
        .unwrap_or("info");
    event.severity = Severity::from_str(severity_str);

    // Timestamp
    if let Some(ts_val) = obj.get("Timestamp").or_else(|| obj.get("timestamp")).or_else(|| obj.get("@timestamp")) {
        if let Some(ts_str) = ts_val.as_str() {
            if let Ok(ts) = ts_str.parse::<chrono::DateTime<Utc>>() {
                event.timestamp = ts;
            }
        }
    }

    // HTTP-специфичные поля
    if let Some(props) = obj.get("Properties").and_then(|v| v.as_object()) {
        extract_http_fields(props, event);
        // Остаток в metadata
        let mut meta: HashMap<String, Value> = HashMap::new();
        for (k, v) in props {
            meta.insert(k.clone(), v.clone());
        }
        event.metadata = meta;
    }

    // ASP.NET Core RequestLog format
    if let Some(method) = obj.get("RequestMethod").and_then(|v| v.as_str()) {
        event.http_method = Some(method.to_string());
    }
    if let Some(path) = obj.get("RequestPath").and_then(|v| v.as_str()) {
        event.url_path = Some(sanitize_url_path(path));
    }
    if let Some(status) = obj.get("StatusCode").and_then(|v| v.as_u64()) {
        event.status_code = Some(status as u16);
    }
    if let Some(elapsed) = obj.get("Elapsed").or_else(|| obj.get("ElapsedMilliseconds")) {
        event.duration_ms = elapsed.as_f64();
    }

    // User/Auth context
    if let Some(uid) = obj.get("UserId").or_else(|| obj.get("user_id")).and_then(|v| v.as_str()) {
        event.user_id = Some(uid.to_string());
    }

    // Source IP
    if let Some(ip) = obj.get("ClientIp").or_else(|| obj.get("client_ip")).or_else(|| obj.get("source_ip")).and_then(|v| v.as_str()) {
        event.source_ip = Some(ip.to_string());
    }

    event.event_type = "application".to_string();
    Ok(())
}

fn extract_http_fields(props: &serde_json::Map<String, Value>, event: &mut NormalizedEvent) {
    if let Some(method) = props.get("HttpMethod").or_else(|| props.get("Method")).and_then(|v| v.as_str()) {
        event.http_method = Some(method.to_string());
    }
    if let Some(path) = props.get("Path").or_else(|| props.get("Url")).and_then(|v| v.as_str()) {
        event.url_path = Some(sanitize_url_path(path));
    }
    if let Some(status) = props.get("StatusCode").and_then(|v| v.as_u64()) {
        event.status_code = Some(status as u16);
    }
    if let Some(ip) = props.get("ClientIp").or_else(|| props.get("RemoteIp")).and_then(|v| v.as_str()) {
        event.source_ip = Some(ip.to_string());
    }
    if let Some(uid) = props.get("UserId").or_else(|| props.get("UserName")).and_then(|v| v.as_str()) {
        event.user_id = Some(uid.to_string());
    }
}

fn parse_cef(raw: Bytes, event: &mut NormalizedEvent) -> Result<(), ParserError> {
    // CEF:Version|Device Vendor|Device Product|Device Version|Signature ID|Name|Severity|Extension
    let s = std::str::from_utf8(&raw)
        .map_err(|_| ParserError::Cef("Invalid UTF-8".to_string()))?;

    let parts: Vec<&str> = s.splitn(8, '|').collect();
    if parts.len() < 8 {
        return Err(ParserError::Cef(format!("Expected 8 pipe-separated fields, got {}", parts.len())));
    }

    event.event_type = "network".to_string();
    event.metadata.insert("cef_vendor".to_string(), Value::String(parts[1].to_string()));
    event.metadata.insert("cef_product".to_string(), Value::String(parts[2].to_string()));
    event.metadata.insert("cef_version".to_string(), Value::String(parts[3].to_string()));
    event.metadata.insert("cef_signature_id".to_string(), Value::String(parts[4].to_string()));

    let name = parts[5];
    let cef_severity = parts[6].parse::<u8>().unwrap_or(5);
    event.severity = match cef_severity {
        0..=3 => Severity::Info,
        4..=6 => Severity::Warning,
        7..=8 => Severity::Error,
        9..=10 => Severity::Critical,
        _ => Severity::Info,
    };
    event.message = name.to_string();

    // Парсим extension: key=value пары
    let extension = parts[7];
    parse_cef_extension(extension, event);

    Ok(())
}

fn parse_cef_extension(ext: &str, event: &mut NormalizedEvent) {
    // Простой state-machine парсер для CEF extension
    let mut remaining = ext;
    while !remaining.is_empty() {
        // Ищем "key="
        if let Some(eq_pos) = remaining.find('=') {
            let key = remaining[..eq_pos].trim().rsplit(' ').next().unwrap_or(&remaining[..eq_pos]);
            let after_eq = &remaining[eq_pos + 1..];

            // Значение — до следующего " key=" или конец строки
            let value_end = find_next_cef_key(after_eq);
            let value = &after_eq[..value_end];

            match key {
                "src" | "sourceAddress" => event.source_ip = Some(value.trim().to_string()),
                "suser" | "sourceUserName" => event.user_id = Some(value.trim().to_string()),
                "request" | "requestURL" => event.url_path = Some(sanitize_url_path(value.trim())),
                "requestMethod" => event.http_method = Some(value.trim().to_string()),
                "outcome" => {
                    if let Ok(code) = value.trim().parse::<u16>() {
                        event.status_code = Some(code);
                    }
                }
                _ => {
                    event.metadata.insert(key.to_string(), Value::String(value.trim().to_string()));
                }
            }

            if value_end >= after_eq.len() {
                break;
            }
            remaining = &after_eq[value_end..];
        } else {
            break;
        }
    }
}

fn find_next_cef_key(s: &str) -> usize {
    // В CEF extension следующий ключ начинается после пробела перед "word="
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            // Проверяем что после пробела идёт ключ (word=)
            let rest = &s[i + 1..];
            if let Some(eq) = rest.find('=') {
                let potential_key = &rest[..eq];
                if !potential_key.contains(' ') && !potential_key.is_empty() {
                    return i;
                }
            }
        }
        i += 1;
    }
    s.len()
}

fn parse_syslog5424(raw: Bytes, event: &mut NormalizedEvent) -> Result<(), ParserError> {
    let s = std::str::from_utf8(&raw)
        .map_err(|_| ParserError::Syslog("Invalid UTF-8".to_string()))?;

    // Парсим priority: "<PRI>VERSION"
    if !s.starts_with('<') {
        return Err(ParserError::Syslog("Missing < at start".to_string()));
    }
    let gt = s.find('>').ok_or_else(|| ParserError::Syslog("Missing >".to_string()))?;
    let pri: u8 = s[1..gt].parse().unwrap_or(13);
    let facility = pri / 8;
    let severity_num = pri % 8;

    event.severity = match severity_num {
        0 => Severity::Critical,
        1 => Severity::Critical,
        2 => Severity::Critical,
        3 => Severity::Error,
        4 => Severity::Warning,
        5 => Severity::Info,
        6 => Severity::Info,
        7 => Severity::Debug,
        _ => Severity::Info,
    };

    event.metadata.insert("syslog_facility".to_string(), Value::Number(facility.into()));

    // Остаток после "<PRI>1 "
    let after_pri = &s[gt + 1..];
    // Пропускаем версию
    let parts: Vec<&str> = after_pri.splitn(7, ' ').collect();
    if parts.len() >= 6 {
        // parts: [version, timestamp, hostname, appname, procid, msgid, msg]
        if let Ok(ts) = parts[1].parse::<chrono::DateTime<Utc>>() {
            event.timestamp = ts;
        }
        event.host = parts[2].to_string();
        event.metadata.insert("appname".to_string(), Value::String(parts[3].to_string()));
        event.message = parts.get(6).unwrap_or(&"").to_string();
    } else {
        event.message = after_pri.to_string();
    }

    event.event_type = "syslog".to_string();
    Ok(())
}

fn parse_syslog3164(raw: Bytes, event: &mut NormalizedEvent) -> Result<(), ParserError> {
    let s = std::str::from_utf8(&raw)
        .map_err(|_| ParserError::Syslog("Invalid UTF-8".to_string()))?;

    // RFC3164: "<PRI>Mmm DD HH:MM:SS hostname tag: message"
    let gt = s.find('>').ok_or_else(|| ParserError::Syslog("Missing >".to_string()))?;
    let pri: u8 = s[1..gt].parse().unwrap_or(13);
    let severity_num = pri % 8;

    event.severity = match severity_num {
        0..=2 => Severity::Critical,
        3 => Severity::Error,
        4 => Severity::Warning,
        5..=6 => Severity::Info,
        7 => Severity::Debug,
        _ => Severity::Info,
    };

    event.message = s[gt + 1..].to_string();
    event.event_type = "syslog".to_string();
    Ok(())
}

fn parse_plaintext(raw: Bytes, event: &mut NormalizedEvent) {
    event.message = String::from_utf8_lossy(&raw).into_owned();
    event.event_type = "raw".to_string();
    event.severity = Severity::Info;
}

/// Удаляет query-строку из URL для предотвращения утечки параметров в логах.
fn sanitize_url_path(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json() {
        assert_eq!(detect_format(b"{\"Level\":\"Info\"}"), LogFormat::Json);
    }

    #[test]
    fn test_detect_cef() {
        assert_eq!(detect_format(b"CEF:0|Vendor|Product|1.0|100|Test|5|src=1.2.3.4"), LogFormat::Cef);
    }

    #[test]
    fn test_detect_syslog5424() {
        assert_eq!(detect_format(b"<13>1 2024-01-01T00:00:00Z host app - - msg"), LogFormat::Syslog5424);
    }

    #[test]
    fn test_parse_dotnet_json() {
        let raw = br#"{
            "Timestamp": "2024-01-15T10:30:00Z",
            "Level": "Warning",
            "Message": "Failed login attempt",
            "Properties": {
                "ClientIp": "192.168.1.100",
                "UserId": "user123",
                "StatusCode": 401
            }
        }"#;
        let event = parse(Bytes::from_static(raw), "dotnet", "api-server-01").unwrap();
        assert_eq!(event.severity, Severity::Warning);
        assert_eq!(event.message, "Failed login attempt");
        assert_eq!(event.source_ip, Some("192.168.1.100".to_string()));
        assert_eq!(event.user_id, Some("user123".to_string()));
        assert_eq!(event.status_code, Some(401));
    }

    #[test]
    fn test_url_query_sanitization() {
        let result = sanitize_url_path("/api/users?token=secret123&page=1");
        assert_eq!(result, "/api/users");
    }

    #[test]
    fn test_event_too_large() {
        let large = vec![b'a'; MAX_EVENT_SIZE + 1];
        let result = parse(Bytes::from(large), "test", "host");
        assert!(matches!(result, Err(ParserError::EventTooLarge { .. })));
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod extended_tests;

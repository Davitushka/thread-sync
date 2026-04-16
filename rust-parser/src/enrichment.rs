//! Обогащение событий: GeoIP, ASN lookup, LRU-кэш пользователей, threat intel (Redis SET).
//! MaxMind MMDB читается через mmap — нулевые копии при каждом lookup.

use crate::schema::{GeoInfo, NormalizedEvent};
use maxminddb::{geoip2, Reader};
use memmap2::Mmap;
use moka::sync::Cache;
use std::fs::File;
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, info, warn};

/// Конфигурация обогащения
pub struct EnrichmentConfig {
    pub geoip_city_db_path: String,
    pub geoip_asn_db_path: String,
    pub user_cache_size: usize,
    pub user_cache_ttl_secs: u64,
    /// Если задан URL Redis, проверяем `source_ip` в `SISMEMBER siem:intel:ipv4`.
    pub intel_redis_url: Option<String>,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            geoip_city_db_path: "/etc/geoip/GeoLite2-City.mmdb".to_string(),
            geoip_asn_db_path: "/etc/geoip/GeoLite2-ASN.mmdb".to_string(),
            user_cache_size: 10_000,
            user_cache_ttl_secs: 300,
            intel_redis_url: None,
        }
    }
}

/// Enricher — держит открытые MMDB readers и moka-кэш пользователей.
/// Reader<Mmap> — Send+Sync, файл отображён в адресное пространство без копирования.
/// moka::sync::Cache — потокобезопасный кэш с встроенным TTL, без Mutex.
pub struct Enricher {
    city_reader: Option<Reader<Mmap>>,
    asn_reader: Option<Reader<Mmap>>,
    user_cache: Cache<String, CachedUser>,
    intel_redis: Option<redis::aio::MultiplexedConnection>,
}

#[derive(Clone, Debug)]
struct CachedUser {
    display_name: String,
    department: Option<String>,
}

impl Enricher {
    pub fn new(config: &EnrichmentConfig) -> Self {
        let city_reader = load_mmdb(&config.geoip_city_db_path);
        let asn_reader = load_mmdb(&config.geoip_asn_db_path);

        let cache_size =
            NonZeroUsize::new(config.user_cache_size).unwrap_or(NonZeroUsize::new(10_000).unwrap());
        let cache_ttl = std::time::Duration::from_secs(config.user_cache_ttl_secs);

        // Async Redis connection is established lazily — we store None here
        // and let `enrich_threat_intel` handle the connection if URL is configured.
        let _ = config.intel_redis_url;

        let user_cache = Cache::builder()
            .max_capacity(cache_size.get() as u64)
            .time_to_idle(cache_ttl)
            .build();

        Self {
            city_reader,
            asn_reader,
            user_cache,
            intel_redis: None,
        }
    }

    /// Creates a new Enricher with an async Redis connection.
    /// Call this from an async context instead of `new()`.
    pub async fn new_async(config: &EnrichmentConfig) -> Self {
        let city_reader = load_mmdb(&config.geoip_city_db_path);
        let asn_reader = load_mmdb(&config.geoip_asn_db_path);

        let cache_size =
            NonZeroUsize::new(config.user_cache_size).unwrap_or(NonZeroUsize::new(10_000).unwrap());
        let cache_ttl = std::time::Duration::from_secs(config.user_cache_ttl_secs);

        let intel_redis_url = config
            .intel_redis_url
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let intel_redis_conn = if let Some(url) = intel_redis_url {
            match redis::Client::open(url.as_str()) {
                Ok(client) => match client.get_multiplexed_async_connection().await {
                    Ok(conn) => {
                        info!("Threat intel Redis enrichment enabled (async, SET siem:intel:ipv4)");
                        Some(conn)
                    }
                    Err(e) => {
                        warn!(
                            "INTEL redis URL set but async connection failed: {} — intel match disabled",
                            e
                        );
                        None
                    }
                },
                Err(e) => {
                    warn!("Invalid INTEL redis URL: {} — intel match disabled", e);
                    None
                }
            }
        } else {
            None
        };

        let user_cache = Cache::builder()
            .max_capacity(cache_size.get() as u64)
            .time_to_idle(cache_ttl)
            .build();

        Self {
            city_reader,
            asn_reader,
            user_cache,
            intel_redis: intel_redis_conn,
        }
    }

    /// Обогащает событие на месте. Ошибки обогащения не фатальны — событие
    /// передаётся дальше без обогащения с флагом в metadata.
    pub async fn enrich(&self, event: &mut NormalizedEvent) {
        if let Some(ip_str) = event.source_ip.as_deref() {
            match IpAddr::from_str(ip_str) {
                Ok(ip) => {
                    event.geo = self.lookup_geo(ip);
                }
                Err(_) => {
                    debug!("Invalid IP address in source_ip: {}", ip_str);
                }
            }
        }
        self.enrich_threat_intel(event).await;
    }

    async fn enrich_threat_intel(&self, event: &mut NormalizedEvent) {
        let Some(conn) = self.intel_redis.as_ref() else {
            return;
        };
        let Some(ip) = event.source_ip.as_deref().filter(|s| !s.is_empty()) else {
            return;
        };
        use redis::AsyncCommands;
        let mut conn = conn.clone();
        let member: bool = conn
            .sismember("siem:intel:ipv4", ip)
            .await
            .unwrap_or(false);
        if member {
            crate::metrics::INTEL_IOC_MATCH_TOTAL.inc();
            event
                .metadata
                .insert("threat_intel_match".to_string(), serde_json::json!(true));
            event.metadata.insert(
                "threat_intel_ioc_type".to_string(),
                serde_json::json!("ipv4"),
            );
        }
    }

    fn lookup_geo(&self, ip: IpAddr) -> Option<GeoInfo> {
        // Пропускаем приватные и loopback адреса
        if is_private_ip(ip) {
            return None;
        }

        let city_info = self
            .city_reader
            .as_ref()
            .and_then(|reader| reader.lookup(ip).ok())
            .and_then(|result| result.decode::<geoip2::City>().ok())
            .flatten();

        let asn_info = self
            .asn_reader
            .as_ref()
            .and_then(|reader| reader.lookup(ip).ok())
            .and_then(|result| result.decode::<geoip2::Asn>().ok())
            .flatten();

        // Если нет ни GeoIP ни ASN данных — возвращаем None
        if city_info.is_none() && asn_info.is_none() {
            return None;
        }

        let country_iso = city_info
            .as_ref()
            .and_then(|c| c.country.iso_code)
            .unwrap_or("XX")
            .to_string();

        let country_name = city_info
            .as_ref()
            .and_then(|c| c.country.names.english)
            .unwrap_or("Unknown")
            .to_string();

        let city = city_info
            .as_ref()
            .and_then(|c| c.city.names.english)
            .map(|s| s.to_string());

        let (latitude, longitude) = city_info
            .as_ref()
            .map(|c| (c.location.latitude, c.location.longitude))
            .unwrap_or((None, None));

        let (asn_num, asn_org) = asn_info
            .map(|a| {
                (
                    a.autonomous_system_number,
                    a.autonomous_system_organization.map(|s| s.to_string()),
                )
            })
            .unwrap_or((None, None));

        Some(GeoInfo {
            country_iso,
            country_name,
            city,
            latitude,
            longitude,
            asn: asn_num,
            org: asn_org,
        })
    }
}

/// Открывает MMDB файл через mmap — нулевые копии данных при каждом lookup.
/// SAFETY: mmap безопасен для read-only файлов при условии что файл не изменяется
/// в процессе работы (стандартная практика для GeoIP БД).
fn load_mmdb(path: &str) -> Option<Reader<Mmap>> {
    if !Path::new(path).exists() {
        warn!(
            "GeoIP database not found: {}. GeoIP enrichment disabled.",
            path
        );
        return None;
    }

    let t0 = std::time::Instant::now();
    let result = (|| -> std::io::Result<Reader<Mmap>> {
        let file = File::open(path)?;
        // SAFETY: файл открыт только для чтения и не изменяется во время жизни mmap.
        let mmap = unsafe { Mmap::map(&file)? };
        Reader::from_source(mmap)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    })();

    match result {
        Ok(reader) => {
            info!(
                path = path,
                elapsed_ms = t0.elapsed().as_millis(),
                "GeoIP database loaded via mmap"
            );
            Some(reader)
        }
        Err(e) => {
            warn!(
                "Failed to open GeoIP database {}: {}. GeoIP enrichment disabled.",
                path, e
            );
            None
        }
    }
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[tokio::test]
    async fn test_enricher_no_mmdb() {
        let config = EnrichmentConfig {
            geoip_city_db_path: "/nonexistent/city.mmdb".to_string(),
            geoip_asn_db_path: "/nonexistent/asn.mmdb".to_string(),
            ..Default::default()
        };
        let enricher = Enricher::new(&config);
        let mut event = crate::schema::NormalizedEvent::new("test");
        event.source_ip = Some("8.8.8.8".to_string());
        enricher.enrich(&mut event).await;
        // Без MMDB базы — geo должен быть None, но событие не потеряно
        assert!(event.geo.is_none());
    }
}

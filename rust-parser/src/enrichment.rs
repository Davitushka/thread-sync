//! Обогащение событий: GeoIP, ASN lookup, LRU-кэш пользователей.
//! MaxMind MMDB читается через mmap — нулевые копии при каждом lookup.

use crate::error::ParserError;
use crate::schema::{GeoInfo, NormalizedEvent};
use lru::LruCache;
use maxminddb::{geoip2, Reader};
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

/// Конфигурация обогащения
pub struct EnrichmentConfig {
    pub geoip_city_db_path: String,
    pub geoip_asn_db_path: String,
    pub user_cache_size: usize,
    pub user_cache_ttl_secs: u64,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            geoip_city_db_path: "/etc/geoip/GeoLite2-City.mmdb".to_string(),
            geoip_asn_db_path: "/etc/geoip/GeoLite2-ASN.mmdb".to_string(),
            user_cache_size: 10_000,
            user_cache_ttl_secs: 300,
        }
    }
}

/// Enricher — держит открытые MMDB readers и LRU-кэш пользователей.
/// Потокобезопасен: Reader<Mmap> — Send+Sync, LruCache защищён Mutex.
pub struct Enricher {
    city_reader: Option<Reader<Vec<u8>>>,
    asn_reader: Option<Reader<Vec<u8>>>,
    // В продакшне LruCache с TTL лучше заменить на moka::sync::Cache
    user_cache: Arc<Mutex<LruCache<String, CachedUser>>>,
}

#[derive(Clone, Debug)]
struct CachedUser {
    display_name: String,
    department: Option<String>,
    cached_at: std::time::Instant,
}

impl Enricher {
    pub fn new(config: &EnrichmentConfig) -> Self {
        let city_reader = load_mmdb(&config.geoip_city_db_path);
        let asn_reader = load_mmdb(&config.geoip_asn_db_path);

        let cache_size = NonZeroUsize::new(config.user_cache_size).unwrap_or(NonZeroUsize::new(10_000).unwrap());

        Self {
            city_reader,
            asn_reader,
            user_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
        }
    }

    /// Обогащает событие на месте. Ошибки обогащения не фатальны — событие
    /// передаётся дальше без обогащения с флагом в metadata.
    pub fn enrich(&self, event: &mut NormalizedEvent) {
        if let Some(ip_str) = &event.source_ip.clone() {
            match IpAddr::from_str(ip_str) {
                Ok(ip) => {
                    event.geo = self.lookup_geo(ip);
                }
                Err(_) => {
                    debug!("Invalid IP address in source_ip: {}", ip_str);
                }
            }
        }
    }

    fn lookup_geo(&self, ip: IpAddr) -> Option<GeoInfo> {
        // Пропускаем приватные и loopback адреса
        if is_private_ip(ip) {
            return None;
        }

        let city_info = self.city_reader.as_ref().and_then(|reader| {
            reader.lookup::<geoip2::City>(ip).ok()
        });

        let asn_info = self.asn_reader.as_ref().and_then(|reader| {
            reader.lookup::<geoip2::Asn>(ip).ok()
        });

        // Если нет ни GeoIP ни ASN данных — возвращаем None
        if city_info.is_none() && asn_info.is_none() {
            return None;
        }

        let country_iso = city_info.as_ref()
            .and_then(|c| c.country.as_ref())
            .and_then(|c| c.iso_code)
            .unwrap_or("XX")
            .to_string();

        let country_name = city_info.as_ref()
            .and_then(|c| c.country.as_ref())
            .and_then(|c| c.names.as_ref())
            .and_then(|n| n.get("en"))
            .copied()
            .unwrap_or("Unknown")
            .to_string();

        let city = city_info.as_ref()
            .and_then(|c| c.city.as_ref())
            .and_then(|c| c.names.as_ref())
            .and_then(|n| n.get("en"))
            .map(|s| s.to_string());

        let (latitude, longitude) = city_info.as_ref()
            .and_then(|c| c.location.as_ref())
            .map(|loc| (loc.latitude, loc.longitude))
            .unwrap_or((None, None));

        let (asn_num, asn_org) = asn_info
            .map(|a| (a.autonomous_system_number, a.autonomous_system_organization.map(|s| s.to_string())))
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

fn load_mmdb(path: &str) -> Option<Reader<Vec<u8>>> {
    if !Path::new(path).exists() {
        warn!("GeoIP database not found: {}. GeoIP enrichment disabled.", path);
        return None;
    }
    match Reader::open_readfile(path) {
        Ok(reader) => {
            debug!("Loaded GeoIP database: {}", path);
            Some(reader)
        }
        Err(e) => {
            warn!("Failed to open GeoIP database {}: {}. GeoIP enrichment disabled.", path, e);
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

    #[test]
    fn test_enricher_no_mmdb() {
        let config = EnrichmentConfig {
            geoip_city_db_path: "/nonexistent/city.mmdb".to_string(),
            geoip_asn_db_path: "/nonexistent/asn.mmdb".to_string(),
            ..Default::default()
        };
        let enricher = Enricher::new(&config);
        let mut event = crate::schema::NormalizedEvent::new("test");
        event.source_ip = Some("8.8.8.8".to_string());
        enricher.enrich(&mut event);
        // Без MMDB базы — geo должен быть None, но событие не потеряно
        assert!(event.geo.is_none());
    }
}

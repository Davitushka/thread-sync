use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use siem_parser::{
    enrichment::{Enricher, EnrichmentConfig},
    normalizer::NormalizationPipeline,
};

fn make_pipeline() -> NormalizationPipeline {
    let enricher = Enricher::new(&EnrichmentConfig {
        geoip_city_db_path: "/nonexistent".to_string(),
        geoip_asn_db_path: "/nonexistent".to_string(),
        ..Default::default()
    });
    NormalizationPipeline::new(enricher, true, false)
}

fn bench_parse_json(c: &mut Criterion) {
    let pipeline = make_pipeline();

    let small_event = serde_json::json!({
        "Timestamp": "2024-01-15T10:30:00Z",
        "Level": "Warning",
        "Message": "Failed login attempt for user admin@example.com",
        "Properties": {
            "ClientIp": "203.0.113.42",
            "UserId": "user123",
            "StatusCode": 401,
            "RequestPath": "/api/auth/login"
        }
    }).to_string();

    let large_event = serde_json::json!({
        "Timestamp": "2024-01-15T10:30:00Z",
        "Level": "Error",
        "Message": "SQL query failed: SELECT * FROM users WHERE id = 1 AND status = 'active' ORDER BY created_at DESC LIMIT 100",
        "Properties": {
            "ClientIp": "203.0.113.42",
            "UserId": "admin@company.com",
            "StatusCode": 500,
            "RequestPath": "/api/reports/export?token=eyJhbGciOiJSUzI1NiJ9.payload.sig",
            "ElapsedMilliseconds": 1523,
            "TraceId": "abc123def456",
            "SpanId": "xyz789",
            "Database": "production",
            "Schema": "public",
            "Table": "users",
            "RowCount": 0,
            "ConnectionString": "Server=db;Database=prod;User=app;Password=secret123"
        }
    }).to_string();

    let mut group = c.benchmark_group("parse_json");

    for (name, payload) in [("small_1kb", &small_event), ("large_4kb", &large_event)] {
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(BenchmarkId::new("dotnet", name), payload, |b, p| {
            b.iter(|| {
                pipeline.process(
                    black_box(Bytes::from(p.clone())),
                    "dotnet",
                    "bench-host",
                )
            });
        });
    }

    group.finish();
}

fn bench_pii_masking(c: &mut Criterion) {
    use siem_parser::pii::mask_pii_owned;

    let test_cases = vec![
        ("no_pii", "Normal log message without any personal information.".to_string()),
        ("with_email", "User john.doe@example.com logged in from 192.168.1.100".to_string()),
        ("with_token", "Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyMTIzIn0.signature".to_string()),
        ("heavy_pii", "User +7-495-123-4567 (john@corp.com) paid with card 4111111111111111, token: Bearer eyABC.DEF.GHI".to_string()),
    ];

    let mut group = c.benchmark_group("pii_masking");
    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, i| {
            b.iter(|| mask_pii_owned(black_box(i.clone())));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse_json, bench_pii_masking);
criterion_main!(benches);

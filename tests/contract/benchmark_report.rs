use cockpit_runner::benchmark::{BenchmarkConfig, run};

#[test]
fn benchmark_report_is_reproducible_and_contains_capacity_dimensions() {
    let report = run(BenchmarkConfig {
        scenario_path: "scenarios/smoke-in-cockpit.yaml".to_string(),
        ticks: 20,
        active_entities: 1_000,
        events_per_minute: 10_000,
    })
    .expect("benchmark runs");
    assert_eq!(report.seed, 42);
    assert_eq!(report.active_entities, 1_000);
    assert_eq!(report.events_per_minute, 10_000);
    assert_eq!(report.ticks, 20);
    assert!(report.p95_tick_ms >= 0.0);
    assert!(report.p99_tick_ms >= report.p95_tick_ms);
    assert!(report.recording_bytes > 0);
    assert!(report.synthetic_workload_hash.starts_with("sha256:"));
}

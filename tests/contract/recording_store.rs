use cockpit_agent::{HumanDecision, HumanTurnEvidence, OpenWorldCheckpoint, OpenWorldRuntime};
use cockpit_recording::{
    AsyncRecordingSink, RecordedAuditPageRequest, Recording, RecordingQueueOutcome,
    RecordingQueuePolicy, RecordingStore, RunProvenance, run_rule_agent_recording,
};
use cockpit_scenario::load_scenario;
use rusqlite::params;
use sha2::{Digest, Sha256};

fn recording_database(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cockpit-recording-{name}-{}.sqlite",
        uuid::Uuid::new_v4()
    ))
}

fn replacement_recording(recording: &Recording) -> Recording {
    let mut replacement = recording.clone();
    replacement.ticks[0].snapshot_hash =
        format!("replacement-{}", replacement.ticks[0].snapshot_hash);
    replacement
}

#[test]
fn sqlite_recording_round_trip_preserves_tick_evidence() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("sqlite-run", scenario, 12).expect("run completes");
    let mut store = RecordingStore::in_memory().expect("store opens");
    store.save(&recording).expect("recording saves");
    let restored = store.load("sqlite-run").expect("recording loads");

    assert_eq!(restored.scenario_hash, recording.scenario_hash);
    assert_eq!(restored.ticks.len(), recording.ticks.len());
    assert_eq!(
        restored.final_snapshot_hash(),
        recording.final_snapshot_hash()
    );
    assert!(
        restored
            .ticks
            .iter()
            .any(|tick| !tick.tool_calls.is_empty())
    );
}

#[test]
fn payload_gc_keeps_committed_payloads_and_removes_superseded_payloads() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let mut recording = run_rule_agent_recording("gc-run", scenario, 4).expect("run completes");
    let mut store = RecordingStore::in_memory().expect("store opens");
    store.save(&recording).expect("initial recording saves");

    recording.ticks.truncate(1);
    store.save(&recording).expect("replacement recording saves");
    let report = store.collect_garbage().expect("gc succeeds");
    assert!(report.removed_orphaned_payloads > 0, "{report:?}");
    assert_eq!(
        store.load("gc-run").expect("committed recording loads"),
        recording
    );
    assert_eq!(
        store.collect_garbage().expect("second gc succeeds"),
        cockpit_recording::PayloadGcReport::default()
    );
}

#[test]
fn payload_publish_failure_does_not_create_an_active_recording() {
    let database = recording_database("payload-publish-failure");
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("payload-publish-failure", scenario, 1).expect("run");
    let payload = serde_json::to_vec(
        &serde_json::to_value(&recording.ticks[0]).expect("tick converts to canonical JSON"),
    )
    .expect("canonical tick serializes");
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    let hash = format!("sha256:{:x}", hasher.finalize());
    let payloads = cockpit_recording::PayloadStore::new(database.with_extension("payloads"))
        .expect("payload store");
    std::fs::create_dir_all(payloads.path_for(&hash)).expect("conflicting payload directory");

    let mut store = RecordingStore::open(&database_text).expect("store opens");
    let error = store
        .save(&recording)
        .expect_err("payload publish must fail");
    assert!(error.to_string().contains("I/O"));
    assert!(store.load("payload-publish-failure").is_err());
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
    let _ = std::fs::remove_file(database);
}

#[test]
fn sql_commit_failure_keeps_the_previous_generation_and_gc_reclaims_payloads() {
    let database = recording_database("commit-failure");
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("commit-failure", scenario, 2).expect("run");
    let replacement = replacement_recording(&recording);
    let mut store = RecordingStore::open(&database_text).expect("store opens");
    store.save(&recording).expect("initial save");
    let connection = rusqlite::Connection::open(&database).expect("database opens");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_generation_insert BEFORE INSERT ON recording_generation_ticks
             BEGIN SELECT RAISE(FAIL, 'injected generation insert failure'); END;",
        )
        .expect("trigger installs");
    let error = store.save(&replacement).expect_err("transaction must fail");
    assert!(
        error
            .to_string()
            .contains("injected generation insert failure")
    );
    assert_eq!(
        store
            .load("commit-failure")
            .expect("old active recording")
            .ticks[0]
            .snapshot_hash,
        recording.ticks[0].snapshot_hash
    );
    connection
        .execute_batch("DROP TRIGGER fail_generation_insert;")
        .expect("trigger removes");
    assert!(
        store
            .collect_garbage()
            .expect("gc succeeds")
            .removed_orphaned_payloads
            > 0,
        "payloads published before the rolled-back transaction are reclaimable"
    );
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
    let _ = std::fs::remove_file(database);
}

#[test]
fn cleanup_failure_does_not_report_an_already_published_generation_as_failed() {
    let database = recording_database("cleanup-failure");
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("cleanup-failure", scenario, 2).expect("run");
    let replacement = replacement_recording(&recording);
    let mut store = RecordingStore::open(&database_text).expect("store opens");
    store.save(&recording).expect("initial save");
    let connection = rusqlite::Connection::open(&database).expect("database opens");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_inactive_cleanup BEFORE DELETE ON recording_generation_ticks
             WHEN OLD.generation <> 2
             BEGIN SELECT RAISE(FAIL, 'injected cleanup failure'); END;",
        )
        .expect("trigger installs");
    store
        .save(&replacement)
        .expect("active generation publishes");
    assert_eq!(
        store
            .load("cleanup-failure")
            .expect("new active recording")
            .ticks[0]
            .snapshot_hash,
        replacement.ticks[0].snapshot_hash
    );
    let stale_rows: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM recording_generation_ticks WHERE run_id = ?1 AND generation = 1",
            params!["cleanup-failure"],
            |row| row.get(0),
        )
        .expect("stale rows count");
    assert!(
        stale_rows > 0,
        "injected cleanup failure leaves only inactive rows"
    );
    connection
        .execute_batch("DROP TRIGGER fail_inactive_cleanup;")
        .expect("trigger removes");
    drop(store);
    let recovered = RecordingStore::open(&database_text).expect("recovery opens");
    assert_eq!(
        recovered
            .load("cleanup-failure")
            .expect("active recording")
            .ticks[0]
            .snapshot_hash,
        replacement.ticks[0].snapshot_hash
    );
    let connection = rusqlite::Connection::open(&database).expect("database opens");
    let stale_rows: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM recording_generation_ticks WHERE run_id = ?1 AND generation = 1",
            params!["cleanup-failure"],
            |row| row.get(0),
        )
        .expect("recovered stale rows count");
    assert_eq!(stale_rows, 0, "writable open retries inactive cleanup");
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
    let _ = std::fs::remove_file(database);
}

#[test]
fn payload_metadata_size_matches_the_redacted_object_on_disk() {
    let database = std::env::temp_dir().join(format!(
        "cockpit-recording-payload-size-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording =
        run_rule_agent_recording("payload-size-run", scenario, 3).expect("run completes");
    let mut store = RecordingStore::open(&database_text).expect("store opens");
    store.save(&recording).expect("recording saves");
    drop(store);

    let connection = rusqlite::Connection::open(&database).expect("database opens");
    let (payload_hash, payload_size): (String, usize) = connection
        .query_row(
            "SELECT payload_hash, payload_size FROM recording_generation_ticks
             WHERE run_id = ?1 ORDER BY tick LIMIT 1",
            params!["payload-size-run"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("payload metadata loads");
    let payloads = cockpit_recording::PayloadStore::new(database.with_extension("payloads"))
        .expect("payload store opens");
    let persisted = payloads.get(&payload_hash).expect("payload reads");
    assert_eq!(payload_size, persisted.len());

    let _ = std::fs::remove_file(&database);
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
}

#[test]
fn generation_recovery_never_publishes_or_retains_an_interrupted_generation() {
    let database = std::env::temp_dir().join(format!(
        "cockpit-recording-generation-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("generation-run", scenario, 3).expect("run completes");
    let mut store = RecordingStore::open(&database_text).expect("store opens");
    store.save(&recording).expect("generation publishes");
    drop(store);

    // Model a process that finished writing a candidate generation's rows but
    // died before the transaction could switch recordings.active_generation.
    let connection = rusqlite::Connection::open(&database).expect("database opens");
    let active_generation: u64 = connection
        .query_row(
            "SELECT active_generation FROM recordings WHERE run_id = ?1",
            params!["generation-run"],
            |row| row.get(0),
        )
        .expect("active generation");
    connection
        .execute(
            "INSERT INTO recording_generation_ticks (run_id, generation, tick, snapshot_hash, payload_hash, payload_size)
             SELECT run_id, ?2, tick, snapshot_hash, payload_hash, payload_size
             FROM recording_generation_ticks WHERE run_id = ?1 AND generation = ?3",
            params!["generation-run", active_generation + 1, active_generation],
        )
        .expect("stale generation injects");
    drop(connection);

    let recovered = RecordingStore::open(&database_text).expect("recovery opens");
    let restored = recovered
        .load("generation-run")
        .expect("active generation loads");
    assert_eq!(restored.scenario_hash, recording.scenario_hash);
    assert_eq!(restored.ticks.len(), recording.ticks.len());
    assert_eq!(
        restored.final_snapshot_hash(),
        recording.final_snapshot_hash(),
        "recovery retains the previously active generation"
    );
    assert_eq!(
        restored.provenance.rule_policy_hash,
        recording.provenance.rule_policy_hash
    );
    let connection = rusqlite::Connection::open(&database).expect("database reopens");
    let stale_rows: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM recording_generation_ticks WHERE run_id = ?1 AND generation <> ?2",
            params!["generation-run", active_generation],
            |row| row.get(0),
        )
        .expect("stale count");
    assert_eq!(
        stale_rows, 0,
        "startup recovery removes inactive generations"
    );
    let _ = std::fs::remove_file(&database);
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
}

#[test]
fn read_only_store_loads_legacy_tick_schema() {
    let database = std::env::temp_dir().join(format!(
        "cockpit-recording-legacy-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_text = database.to_string_lossy().to_string();
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("legacy-run", scenario, 3).expect("run completes");
    let connection = rusqlite::Connection::open(&database).expect("database opens");
    connection
        .execute_batch(
            "CREATE TABLE recordings (
                run_id TEXT PRIMARY KEY,
                schema_version INTEGER NOT NULL,
                runtime_contract_version INTEGER NOT NULL,
                world_model_version INTEGER NOT NULL,
                application_commit TEXT NOT NULL,
                plugin_hashes_json TEXT NOT NULL,
                scenario_id TEXT NOT NULL,
                scenario_hash TEXT NOT NULL,
                seed INTEGER NOT NULL,
                clock_json TEXT NOT NULL,
                human_turns_json TEXT NOT NULL DEFAULT '[]',
                provenance_json TEXT NOT NULL DEFAULT '{}',
                open_world_checkpoint_json TEXT NOT NULL DEFAULT 'null'
             );
             CREATE TABLE recording_ticks (
                run_id TEXT NOT NULL,
                tick INTEGER NOT NULL,
                snapshot_hash TEXT NOT NULL,
                payload_hash TEXT NOT NULL,
                payload_size INTEGER NOT NULL,
                PRIMARY KEY(run_id, tick)
             );",
        )
        .expect("legacy schema creates");
    connection
        .execute(
            "INSERT INTO recordings (run_id, schema_version, runtime_contract_version, world_model_version, application_commit, plugin_hashes_json, scenario_id, scenario_hash, seed, clock_json, human_turns_json, provenance_json, open_world_checkpoint_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                recording.run_id,
                recording.schema_version,
                recording.runtime_contract_version,
                recording.world_model_version,
                recording.application_commit,
                serde_json::to_string(&recording.plugin_hashes).expect("plugin hashes serialize"),
                recording.scenario_id,
                recording.scenario_hash,
                recording.seed,
                serde_json::to_string(&recording.clock).expect("clock serializes"),
                serde_json::to_string(&recording.human_turns).expect("human turns serialize"),
                serde_json::to_string(&recording.provenance).expect("provenance serializes"),
                serde_json::to_string(&recording.open_world_checkpoint).expect("checkpoint serializes"),
            ],
        )
        .expect("legacy metadata inserts");
    let payloads = cockpit_recording::PayloadStore::new(database.with_extension("payloads"))
        .expect("payload store opens");
    for tick in &recording.ticks {
        let payload = serde_json::to_vec(tick).expect("tick serializes");
        let payload_hash = payloads.put(&payload).expect("payload writes");
        connection
            .execute(
                "INSERT INTO recording_ticks (run_id, tick, snapshot_hash, payload_hash, payload_size)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    recording.run_id,
                    tick.tick,
                    tick.snapshot_hash,
                    payload_hash,
                    payload.len(),
                ],
            )
            .expect("legacy tick inserts");
    }
    drop(connection);

    let restored = RecordingStore::open_read_only(&database_text)
        .expect("read-only legacy store opens")
        .load("legacy-run")
        .expect("legacy recording loads");
    assert_eq!(restored.scenario_hash, recording.scenario_hash);
    assert_eq!(restored.ticks.len(), recording.ticks.len());
    assert_eq!(
        restored.final_snapshot_hash(),
        recording.final_snapshot_hash()
    );

    let _ = std::fs::remove_file(&database);
    let _ = std::fs::remove_dir_all(database.with_extension("payloads"));
}

#[test]
fn sqlite_recording_round_trip_preserves_live_human_turns() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let mut recording = Recording::new("sqlite-live-run", &scenario);
    let mut runtime = OpenWorldRuntime::default();
    runtime.ensure_agent("pilot-1", "protect occupants", 0);
    let world = cockpit_world::Simulation::new("sqlite-live-run", scenario.clone());
    recording.open_world_checkpoint = Some(OpenWorldCheckpoint::capture(&world.snapshot, &runtime));
    recording.provenance = RunProvenance {
        suite_id: Some("release-suite".to_string()),
        split: Some("hiddenRelease".to_string()),
        backend: Some("iota-core-acp".to_string()),
        ..RunProvenance::default()
    };
    recording.push_human_turns(vec![HumanTurnEvidence {
        human_id: "pilot-1".to_string(),
        decision: HumanDecision {
            narrative: "watched the engine panel".to_string(),
            utterance: Some("status check".to_string()),
            ..HumanDecision::default()
        },
        disposition: Default::default(),
        tool_calls: Vec::new(),
        latency_ms: None,
    }]);

    let mut store = RecordingStore::in_memory().expect("store opens");
    store.save(&recording).expect("recording saves");
    let restored = store.load("sqlite-live-run").expect("recording loads");

    assert_eq!(restored.human_turns.len(), 1);
    assert_eq!(restored.provenance, recording.provenance);
    assert_eq!(
        restored.open_world_checkpoint,
        recording.open_world_checkpoint
    );
    assert_eq!(
        restored
            .open_world_checkpoint
            .as_ref()
            .expect("checkpoint restored")
            .runtime
            .sessions
            .len(),
        1
    );
    assert_eq!(restored.human_turns[0][0].human_id, "pilot-1");
    assert_eq!(restored.human_turns[0][0].decision.narrative, "[REDACTED]");
    assert_eq!(
        restored.human_turns[0][0].decision.utterance.as_deref(),
        Some("[REDACTED]")
    );
}

#[test]
fn durable_audit_window_is_tick_scoped_and_redacted() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let mut recording = Recording::new("audit-window-run", &scenario);
    let mut runtime = OpenWorldRuntime::default();
    runtime.ensure_agent("pilot-1", "protect occupants", 0);
    let mut world = cockpit_world::Simulation::new("audit-window-run", scenario);
    recording.open_world_checkpoint = Some(OpenWorldCheckpoint::capture(&world.snapshot, &runtime));
    world.start().expect("world starts");
    recording.push(
        world
            .step_with_state_diffs(Vec::new())
            .expect("tick commits"),
    );
    recording.push_human_turns(vec![HumanTurnEvidence {
        human_id: "pilot-1".to_string(),
        decision: HumanDecision {
            narrative: "private narrative".to_string(),
            utterance: Some("private utterance".to_string()),
            ..HumanDecision::default()
        },
        disposition: Default::default(),
        tool_calls: Vec::new(),
        latency_ms: None,
    }]);
    let mut store = RecordingStore::in_memory().expect("store opens");
    store.save(&recording).expect("recording saves");
    let audit = store
        .load_audit_window("audit-window-run", 0, 0)
        .expect("audit window loads");
    let serialized = serde_json::to_string(&audit).expect("audit serializes");
    assert!(serialized.contains("[REDACTED]"));
    assert!(!serialized.contains("private narrative"));
    assert!(!serialized.contains("private utterance"));
    assert!(
        audit
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence),
        "audit sequences must be strictly increasing"
    );
    let tail_page = store
        .load_audit_page(
            "audit-window-run",
            RecordedAuditPageRequest {
                start_tick: 0,
                end_tick: 0,
                offset: None,
                limit: 1,
                after_sequence: None,
                tail_limit: Some(1),
            },
        )
        .expect("materialized audit tail page loads");
    assert_eq!(tail_page.total_events, audit.len());
    assert_eq!(tail_page.events.len(), 1);
    assert_eq!(tail_page.truncated, audit.len() > 1);
    let first_page = store
        .load_audit_page(
            "audit-window-run",
            RecordedAuditPageRequest {
                start_tick: 0,
                end_tick: 0,
                offset: Some(0),
                limit: 1,
                after_sequence: None,
                tail_limit: None,
            },
        )
        .expect("materialized first page loads");
    let after_sequence = first_page.next_sequence.expect("page has continuation");
    let sequence_page = store
        .load_audit_page(
            "audit-window-run",
            RecordedAuditPageRequest {
                start_tick: 0,
                end_tick: 0,
                offset: None,
                limit: 1,
                after_sequence: Some(after_sequence),
                tail_limit: None,
            },
        )
        .expect("materialized sequence page loads");
    assert_eq!(sequence_page.events[0].sequence, audit[1].sequence);
    assert!(
        serde_json::to_string(&tail_page)
            .expect("page serializes")
            .contains("[REDACTED]")
    );
    assert!(
        store
            .load_audit_window("audit-window-run", 2, 1)
            .expect_err("inverted tick window must fail")
            .to_string()
            .contains("after end")
    );
}

#[test]
fn sustained_async_overload_triggers_bounded_queue_policy() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("overload-run", scenario, 16).expect("run completes");
    assert!(recording.ticks.len() >= 4, "need several steps to overflow");

    // A slow async consumer that never makes progress while the producer keeps
    // pushing: the bounded queue must reject once capacity is exceeded.
    let mut sink = AsyncRecordingSink::new(2, RecordingQueuePolicy::FailRun);
    let mut outcomes = Vec::new();
    for step in recording.ticks.iter().cloned() {
        outcomes.push(sink.push(step));
    }

    assert_eq!(outcomes[0], RecordingQueueOutcome::Enqueued);
    assert_eq!(outcomes[1], RecordingQueueOutcome::Enqueued);
    assert!(
        outcomes[2..]
            .iter()
            .all(|outcome| *outcome == RecordingQueueOutcome::Failed),
        "sustained overload with a lagging consumer must fail closed: {outcomes:?}"
    );
    let health = sink.health();
    assert_eq!(health.capacity, 2);
    assert_eq!(health.enqueued, 2);
    assert!(health.rejected >= 1, "overflow is observable in health");
}

#[test]
fn async_consumer_catching_up_commits_every_step() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let recording = run_rule_agent_recording("drain-run", scenario, 10).expect("run completes");

    // Consumer keeps pace: drain one step after each push so the queue never
    // overflows and every step is eventually committed.
    let mut sink = AsyncRecordingSink::new(2, RecordingQueuePolicy::FailRun);
    for step in recording.ticks.iter().cloned() {
        assert_eq!(sink.push(step), RecordingQueueOutcome::Enqueued);
        sink.drain_one();
    }
    sink.drain_all();
    assert_eq!(
        sink.committed().len(),
        recording.ticks.len(),
        "a consumer that keeps pace commits every step"
    );
}

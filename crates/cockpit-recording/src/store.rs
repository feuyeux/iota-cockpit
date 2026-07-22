use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::replica::{AuthenticatedReplicaStore, PayloadRestoreEvidence};
use crate::{
    RecordedAuditEvent, RecordedAuditPage, RecordedAuditPageRequest, Recording,
    SequencedRecordedAuditEvent,
};

#[derive(Debug, Clone)]
pub struct PayloadStore {
    root: PathBuf,
    replica_root: Option<PathBuf>,
    authenticated_replica: Option<AuthenticatedReplicaStore>,
    restore_enabled: bool,
}

impl PayloadStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, RecordingStoreError> {
        let root = root.into();
        let store = Self {
            replica_root: Some(root.with_extension("replicas")),
            authenticated_replica: None,
            root,
            restore_enabled: true,
        };
        fs::create_dir_all(&store.root)
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        if let Some(replica_root) = &store.replica_root {
            fs::create_dir_all(replica_root)
                .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        }
        Ok(store)
    }

    pub fn put(&self, payload: &[u8]) -> Result<String, RecordingStoreError> {
        self.put_with_size(payload).map(|(hash, _)| hash)
    }

    /// Store the redacted payload and return the content hash plus the exact
    /// byte length written to disk. Metadata must describe persisted bytes,
    /// never the caller's potentially unredacted serialization.
    pub fn put_with_size(&self, payload: &[u8]) -> Result<(String, usize), RecordingStoreError> {
        let payload = redact_payload(payload);
        let payload_size = payload.len();
        let hash = hash_payload(&payload);
        let path = self.path_for(&hash);
        if path.exists() {
            let existing =
                fs::read(&path).map_err(|error| RecordingStoreError::Io(error.to_string()))?;
            if hash_payload(&existing) != hash {
                return Err(RecordingStoreError::PayloadHashMismatch(hash));
            }
            self.publish_replica(&hash, &existing)?;
            if let Some(replica) = &self.authenticated_replica {
                replica.put(&hash, &existing)?;
            }
            return Ok((hash, payload_size));
        }
        fs::create_dir_all(path.parent().expect("payload parent"))
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        // Each writer owns a distinct staging path. A shared `.tmp` name lets
        // concurrent saves corrupt one another before the content-addressed
        // publish rename runs.
        let temp = path.with_file_name(format!(
            ".{digest}.{}.tmp",
            uuid::Uuid::new_v4(),
            digest = hash.strip_prefix("sha256:").unwrap_or(&hash)
        ));
        let mut file =
            fs::File::create(&temp).map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.write_all(&payload)
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.sync_all()
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        if let Err(error) = fs::rename(&temp, &path) {
            // A concurrent writer may have published the identical bytes
            // first. The hash is the identity, so its published object is
            // equivalent to ours.
            if path.exists() {
                let _ = fs::remove_file(&temp);
            } else {
                return Err(RecordingStoreError::Io(error.to_string()));
            }
        }
        self.publish_replica(&hash, &payload)?;
        if let Some(replica) = &self.authenticated_replica {
            replica.put(&hash, &payload)?;
        }
        Ok((hash, payload_size))
    }

    pub fn get(&self, hash: &str) -> Result<Vec<u8>, RecordingStoreError> {
        self.get_with_evidence(hash).map(|(payload, _)| payload)
    }

    pub fn get_with_evidence(
        &self,
        hash: &str,
    ) -> Result<(Vec<u8>, PayloadRestoreEvidence), RecordingStoreError> {
        match self.read_verified(&self.path_for(hash), hash) {
            Ok(bytes) => Ok((
                bytes,
                PayloadRestoreEvidence {
                    hash: hash.to_string(),
                    source: "primary".to_string(),
                    verified: true,
                },
            )),
            Err(primary_error) if self.restore_enabled => {
                if let Some(replica_root) = &self.replica_root {
                    let replica = self.path_at(replica_root, hash);
                    if let Ok(bytes) = self.read_verified(&replica, hash) {
                        self.publish_at(&self.path_for(hash), hash, &bytes)?;
                        return Ok((
                            bytes,
                            PayloadRestoreEvidence {
                                hash: hash.to_string(),
                                source: "localReplica".to_string(),
                                verified: true,
                            },
                        ));
                    }
                }
                if let Some(replica) = &self.authenticated_replica {
                    let (bytes, evidence) = replica.get(hash)?;
                    self.publish_at(&self.path_for(hash), hash, &bytes)?;
                    self.publish_replica(hash, &bytes)?;
                    return Ok((bytes, evidence));
                }
                Err(primary_error)
            }
            Err(error) => Err(error),
        }
    }

    pub fn with_authenticated_replica(
        mut self,
        root: impl Into<PathBuf>,
        key: impl AsRef<[u8]>,
    ) -> Result<Self, RecordingStoreError> {
        self.authenticated_replica = Some(AuthenticatedReplicaStore::new(root, key)?);
        Ok(self)
    }

    pub fn path_for(&self, hash: &str) -> PathBuf {
        self.path_at(&self.root, hash)
    }

    fn path_at(&self, root: &Path, hash: &str) -> PathBuf {
        let digest = hash.strip_prefix("sha256:").unwrap_or(hash);
        root.join(digest.get(..2).unwrap_or("00"))
            .join(format!("{digest}.json"))
    }

    fn read_verified(&self, path: &Path, hash: &str) -> Result<Vec<u8>, RecordingStoreError> {
        let bytes = fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RecordingStoreError::PayloadMissing(hash.to_string())
            } else {
                RecordingStoreError::Io(error.to_string())
            }
        })?;
        if hash_payload(&bytes) != hash {
            return Err(RecordingStoreError::PayloadHashMismatch(hash.to_string()));
        }
        Ok(bytes)
    }

    fn publish_replica(&self, hash: &str, payload: &[u8]) -> Result<(), RecordingStoreError> {
        if let Some(root) = &self.replica_root {
            self.publish_at(&self.path_at(root, hash), hash, payload)?;
        }
        Ok(())
    }

    fn publish_at(
        &self,
        path: &Path,
        hash: &str,
        payload: &[u8],
    ) -> Result<(), RecordingStoreError> {
        if path.exists() {
            self.read_verified(path, hash)?;
            return Ok(());
        }
        fs::create_dir_all(path.parent().expect("payload parent"))
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        let temp = path.with_file_name(format!(
            ".{}.{}.tmp",
            hash.strip_prefix("sha256:").unwrap_or(hash),
            uuid::Uuid::new_v4()
        ));
        let mut file =
            fs::File::create(&temp).map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.write_all(payload)
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.sync_all()
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        if let Err(error) = fs::rename(&temp, path) {
            if path.exists() {
                let _ = fs::remove_file(temp);
                self.read_verified(path, hash)?;
            } else {
                return Err(RecordingStoreError::Io(error.to_string()));
            }
        }
        Ok(())
    }

    fn collect_garbage(
        &self,
        referenced: &BTreeSet<String>,
    ) -> Result<PayloadGcReport, RecordingStoreError> {
        let mut report = self.collect_garbage_at(&self.root, referenced)?;
        if let Some(replica_root) = &self.replica_root {
            let replica_report = self.collect_garbage_at(replica_root, referenced)?;
            report.removed_orphaned_payloads += replica_report.removed_orphaned_payloads;
            report.removed_staged_files += replica_report.removed_staged_files;
        }
        Ok(report)
    }

    fn collect_garbage_at(
        &self,
        root: &Path,
        referenced: &BTreeSet<String>,
    ) -> Result<PayloadGcReport, RecordingStoreError> {
        let mut report = PayloadGcReport::default();
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(report),
            Err(error) => return Err(RecordingStoreError::Io(error.to_string())),
        };
        for entry in entries {
            let entry = entry.map_err(|error| RecordingStoreError::Io(error.to_string()))?;
            if !entry
                .file_type()
                .map_err(|error| RecordingStoreError::Io(error.to_string()))?
                .is_dir()
            {
                continue;
            }
            let files = fs::read_dir(entry.path())
                .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
            for file in files {
                let file = file.map_err(|error| RecordingStoreError::Io(error.to_string()))?;
                let name = file.file_name();
                let name = name.to_string_lossy();
                let path = file.path();
                if name.ends_with(".tmp") {
                    fs::remove_file(path)
                        .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
                    report.removed_staged_files += 1;
                } else if let Some(digest) = name.strip_suffix(".json") {
                    let hash = format!("sha256:{digest}");
                    if !referenced.contains(&hash) {
                        fs::remove_file(path)
                            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
                        report.removed_orphaned_payloads += 1;
                    }
                }
            }
        }
        Ok(report)
    }
}

/// Result of a content-addressed payload cleanup. The operation is idempotent:
/// a second run with no new failed save removes nothing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PayloadGcReport {
    pub removed_orphaned_payloads: usize,
    pub removed_staged_files: usize,
}

fn hash_payload(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("sha256:{:x}", hasher.finalize())
}

const REDACTED: &str = "[REDACTED]";

fn redact_payload(payload: &[u8]) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<Value>(payload) else {
        return payload.to_vec();
    };
    redact_value(&mut value);
    serde_json::to_vec(&value).unwrap_or_else(|_| payload.to_vec())
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(redact_value),
        Value::Object(values) => {
            for (key, value) in values {
                if is_sensitive_key(key) {
                    *value = Value::String(REDACTED.to_string());
                } else {
                    redact_value(value);
                }
            }
        }
        _ => {}
    }
}

fn redact_human_turn_prose(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(redact_human_turn_prose),
        Value::Object(values) => {
            for (key, value) in values {
                if matches!(key.as_str(), "narrative" | "utterance") && value.is_string() {
                    *value = Value::String(REDACTED.to_string());
                } else {
                    redact_human_turn_prose(value);
                }
            }
        }
        _ => {}
    }
}

fn redacted_audit_events(
    recording: &Recording,
    human_turns: Value,
) -> Result<Vec<(u64, u64, String, String)>, RecordingStoreError> {
    let mut recording = recording.clone();
    recording.human_turns = serde_json::from_value(human_turns)?;
    recording
        .audit_window(0, u64::MAX)
        .into_iter()
        .map(|event| {
            let tick = audit_event_tick(&event.event);
            let mut value = serde_json::to_value(event.event)?;
            redact_human_turn_prose(&mut value);
            redact_value(&mut value);
            let event_json = serde_json::to_string(&value)?;
            let event_hash = hash_payload(event_json.as_bytes());
            Ok((event.sequence, tick, event_json, event_hash))
        })
        .collect()
}

fn audit_event_tick(event: &RecordedAuditEvent) -> u64 {
    match event {
        RecordedAuditEvent::WorldEvent { tick, .. }
        | RecordedAuditEvent::ToolCall { tick, .. }
        | RecordedAuditEvent::ActionResult { tick, .. }
        | RecordedAuditEvent::PluginFailure { tick, .. }
        | RecordedAuditEvent::HumanTurn { tick, .. }
        | RecordedAuditEvent::Error { tick, .. } => *tick,
    }
}

/// Serialize a Recording for an external process without persisting secrets,
/// prompts, hidden reasoning, narrative, or utterance prose.
pub fn serialize_redacted_recording(recording: &Recording) -> Result<Vec<u8>, RecordingStoreError> {
    let mut value = serde_json::to_value(recording)?;
    redact_human_turn_prose(&mut value);
    redact_value(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "apikey"
            | "token"
            | "authorization"
            | "password"
            | "secret"
            | "credential"
            | "credentials"
            | "prompt"
            | "reasoning"
            | "hiddenreasoning"
            | "chainofthought"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("password")
        || normalized.ends_with("credential")
        || normalized.ends_with("credentials")
        || normalized.ends_with("prompt")
}

#[derive(Debug, Error)]
pub enum RecordingStoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("recording serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("recording '{0}' was not found")]
    NotFound(String),
    #[error("recording I/O error: {0}")]
    Io(String),
    #[error("payload hash mismatch: {0}")]
    PayloadHashMismatch(String),
    #[error("payload object is missing: {0}")]
    PayloadMissing(String),
    #[error("authenticated replica key must contain at least 16 bytes")]
    InvalidReplicaAuth,
    #[error("audit window start tick {start_tick} is after end tick {end_tick}")]
    InvalidAuditWindow { start_tick: u64, end_tick: u64 },
}

pub struct RecordingStore {
    connection: Connection,
    payloads: PayloadStore,
}

impl RecordingStore {
    pub fn open(path: &str) -> Result<Self, RecordingStoreError> {
        let connection = Connection::open(path)?;
        let payloads = PayloadStore::new(Path::new(path).with_extension("payloads"))?;
        let mut store = Self {
            connection,
            payloads,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn open_with_authenticated_replica(
        path: &str,
        replica_root: impl Into<PathBuf>,
        key: impl AsRef<[u8]>,
    ) -> Result<Self, RecordingStoreError> {
        let connection = Connection::open(path)?;
        let payloads = PayloadStore::new(Path::new(path).with_extension("payloads"))?
            .with_authenticated_replica(replica_root, key)?;
        let mut store = Self {
            connection,
            payloads,
        };
        store.initialize()?;
        Ok(store)
    }

    /// Open an existing recording store without creating tables, directories,
    /// or files. Independent evaluator processes use this to keep the
    /// simulation recording immutable across the evaluation boundary.
    pub fn open_read_only(path: &str) -> Result<Self, RecordingStoreError> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let payloads = PayloadStore {
            root: Path::new(path).with_extension("payloads"),
            replica_root: Some(Path::new(path).with_extension("replicas")),
            authenticated_replica: None,
            restore_enabled: false,
        };
        Ok(Self {
            connection,
            payloads,
        })
    }
    pub fn in_memory() -> Result<Self, RecordingStoreError> {
        let connection = Connection::open_in_memory()?;
        let payloads = PayloadStore::new(
            std::env::temp_dir().join(format!("cockpit-recording-{}", uuid::Uuid::new_v4())),
        )?;
        let mut store = Self {
            connection,
            payloads,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn save(&mut self, recording: &Recording) -> Result<(), RecordingStoreError> {
        let mut human_turns_value = serde_json::to_value(&recording.human_turns)?;
        redact_human_turn_prose(&mut human_turns_value);
        redact_value(&mut human_turns_value);
        let human_turns_json = serde_json::to_string(&human_turns_value)?;
        let audit_events = redacted_audit_events(recording, human_turns_value)?;

        // The writer lock spans payload publication and the active-generation
        // switch, so GC in another process cannot collect an in-flight object.
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let generation = transaction
            .query_row(
                "SELECT active_generation FROM recordings WHERE run_id = ?1",
                params![recording.run_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0_u64)
            .saturating_add(1);
        // A failed file write can leave an unreferenced object, but cannot
        // mutate the active SQL generation. GC reclaims such objects.
        let payloads = recording
            .ticks
            .iter()
            .map(|tick| {
                let payload = serde_json::to_vec(tick)?;
                let (hash, payload_size) = self.payloads.put_with_size(&payload)?;
                Ok((tick, hash, payload_size))
            })
            .collect::<Result<Vec<_>, RecordingStoreError>>()?;
        transaction.execute(
            "INSERT INTO recordings (run_id, schema_version, runtime_contract_version, world_model_version, application_commit, plugin_hashes_json, scenario_id, scenario_hash, seed, clock_json, human_turns_json, provenance_json, open_world_checkpoint_json, active_generation) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(run_id) DO UPDATE SET
                schema_version = excluded.schema_version,
                runtime_contract_version = excluded.runtime_contract_version,
                world_model_version = excluded.world_model_version,
                application_commit = excluded.application_commit,
                plugin_hashes_json = excluded.plugin_hashes_json,
                scenario_id = excluded.scenario_id,
                scenario_hash = excluded.scenario_hash,
                seed = excluded.seed,
                clock_json = excluded.clock_json,
                human_turns_json = excluded.human_turns_json,
                provenance_json = excluded.provenance_json,
                open_world_checkpoint_json = excluded.open_world_checkpoint_json,
                active_generation = excluded.active_generation",
            params![
                recording.run_id,
                recording.schema_version,
                recording.runtime_contract_version,
                recording.world_model_version,
                recording.application_commit,
                serde_json::to_string(&recording.plugin_hashes)?,
                recording.scenario_id,
                recording.scenario_hash,
                recording.seed,
                serde_json::to_string(&recording.clock)?,
                human_turns_json,
                serde_json::to_string(&recording.provenance)?,
                serde_json::to_string(&recording.open_world_checkpoint)?,
                generation,
            ],
        )?;
        for (tick, payload_hash, payload_size) in payloads {
            transaction.execute(
                "INSERT INTO recording_generation_ticks (run_id, generation, tick, snapshot_hash, payload_hash, payload_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    recording.run_id,
                    generation,
                    tick.tick,
                    tick.snapshot_hash,
                    payload_hash,
                    payload_size
                ],
            )?;
        }
        for (sequence, tick, event_json, event_hash) in audit_events {
            transaction.execute(
                "INSERT INTO recording_generation_audit_events (run_id, generation, sequence, tick, event_json, event_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    recording.run_id,
                    generation,
                    sequence,
                    tick,
                    event_json,
                    event_hash,
                ],
            )?;
        }
        transaction.commit()?;
        // A crash before this best-effort cleanup leaves an inactive
        // generation, never a partially visible active recording. `open`
        // repeats the cleanup during recovery.
        // Publication is already durable. Cleanup is recovery work: reporting
        // its failure as a failed save would make callers retry an operation
        // whose new generation is already active.
        let _ = self.connection.execute(
            "DELETE FROM recording_generation_ticks WHERE run_id = ?1 AND generation <> ?2",
            params![recording.run_id, generation],
        );
        let _ = self.connection.execute(
            "DELETE FROM recording_generation_audit_events WHERE run_id = ?1 AND generation <> ?2",
            params![recording.run_id, generation],
        );
        Ok(())
    }

    /// Remove payloads not referenced by any committed recording tick, plus
    /// staging files left by interrupted writers. Call only from the writer
    /// process; read-only evaluator handles intentionally cannot mutate data.
    pub fn collect_garbage(&mut self) -> Result<PayloadGcReport, RecordingStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut statement = transaction.prepare(
            "SELECT DISTINCT ticks.payload_hash
             FROM recording_generation_ticks ticks
             JOIN recordings recordings
               ON recordings.run_id = ticks.run_id
              AND recordings.active_generation = ticks.generation",
        )?;
        let referenced = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?;
        drop(statement);
        let report = self.payloads.collect_garbage(&referenced)?;
        transaction.commit()?;
        Ok(report)
    }

    pub fn load(&self, run_id: &str) -> Result<Recording, RecordingStoreError> {
        let recording_columns = self.table_columns("recordings")?;
        let has_checkpoint = recording_columns.contains("open_world_checkpoint_json");
        let has_active_generation = recording_columns.contains("active_generation");
        let checkpoint_column = if has_checkpoint {
            "open_world_checkpoint_json"
        } else {
            "'null'"
        };
        let generation_column = if has_active_generation {
            "active_generation"
        } else {
            "0"
        };
        let metadata_query = format!(
            "SELECT schema_version, scenario_id, scenario_hash, seed, runtime_contract_version, world_model_version, application_commit, plugin_hashes_json, clock_json, human_turns_json, provenance_json, {checkpoint_column}, {generation_column} FROM recordings WHERE run_id = ?1"
        );
        let metadata = self
            .connection
            .query_row(&metadata_query, params![run_id], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, u64>(12)?,
                ))
            })
            .optional()?
            .ok_or_else(|| RecordingStoreError::NotFound(run_id.to_string()))?;

        let uses_generation_ticks =
            has_active_generation && self.table_exists("recording_generation_ticks")?;
        let tick_query = if uses_generation_ticks {
            "SELECT payload_hash FROM recording_generation_ticks WHERE run_id = ?1 AND generation = ?2 ORDER BY tick ASC"
        } else {
            "SELECT payload_hash FROM recording_ticks WHERE run_id = ?1 ORDER BY tick ASC"
        };
        let mut statement = self.connection.prepare(tick_query)?;
        let payload_hashes = if uses_generation_ticks {
            statement
                .query_map(params![run_id, metadata.12], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map(params![run_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let ticks = payload_hashes
            .into_iter()
            .map(|payload| -> Result<_, RecordingStoreError> {
                Ok(serde_json::from_slice(&self.payloads.get(&payload)?)?)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Recording {
            schema_version: metadata.0,
            runtime_contract_version: metadata.4,
            world_model_version: metadata.5,
            application_commit: metadata.6,
            plugin_hashes: serde_json::from_str(&metadata.7)?,
            run_id: run_id.to_string(),
            scenario_id: metadata.1,
            scenario_hash: metadata.2,
            seed: metadata.3,
            clock: serde_json::from_str(&metadata.8)?,
            ticks,
            human_turns: serde_json::from_str(&metadata.9)?,
            provenance: serde_json::from_str(&metadata.10)?,
            open_world_checkpoint: serde_json::from_str(&metadata.11)?,
        })
    }

    fn table_columns(&self, table: &str) -> Result<BTreeSet<String>, RecordingStoreError> {
        let query = format!("PRAGMA table_info({table})");
        Ok(self
            .connection
            .prepare(&query)?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<_, _>>()?)
    }

    fn table_exists(&self, table: &str) -> Result<bool, RecordingStoreError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                params![table],
                |row| row.get(0),
            )
            .map_err(RecordingStoreError::from)
    }

    /// Read a redacted durable evidence interval for reconnect/audit UIs.
    /// Payload hashes are verified before deserialization.
    pub fn load_audit_window(
        &self,
        run_id: &str,
        start_tick: u64,
        end_tick: u64,
    ) -> Result<Vec<SequencedRecordedAuditEvent>, RecordingStoreError> {
        if start_tick > end_tick {
            return Err(RecordingStoreError::InvalidAuditWindow {
                start_tick,
                end_tick,
            });
        }
        Ok(self.load(run_id)?.audit_window(start_tick, end_tick))
    }

    /// Return a bounded audit page directly from the active generation's
    /// materialized evidence when available. Stores created before the index
    /// keep their hash-verified payload projection as a compatibility path.
    pub fn load_audit_page(
        &self,
        run_id: &str,
        request: RecordedAuditPageRequest,
    ) -> Result<RecordedAuditPage, RecordingStoreError> {
        let RecordedAuditPageRequest {
            start_tick,
            end_tick,
            offset,
            limit,
            after_sequence,
            tail_limit,
        } = request;
        if start_tick > end_tick {
            return Err(RecordingStoreError::InvalidAuditWindow {
                start_tick,
                end_tick,
            });
        }
        let generation = self
            .connection
            .query_row(
                "SELECT active_generation FROM recordings WHERE run_id = ?1",
                params![run_id],
                |row| row.get::<_, u64>(0),
            )
            .optional()?
            .ok_or_else(|| RecordingStoreError::NotFound(run_id.to_string()))?;
        let materialized = self.table_exists("recording_generation_audit_events")?
            && self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM recording_generation_audit_events WHERE run_id = ?1 AND generation = ?2)",
                params![run_id, generation],
                |row| row.get::<_, bool>(0),
            )?;
        if materialized {
            self.load_materialized_audit_page(
                run_id,
                generation,
                start_tick,
                end_tick,
                offset,
                limit,
                after_sequence,
                tail_limit,
            )
        } else {
            Ok(page_audit_events(
                self.load_audit_window(run_id, start_tick, end_tick)?,
                offset,
                limit,
                after_sequence,
                tail_limit,
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn load_materialized_audit_page(
        &self,
        run_id: &str,
        generation: u64,
        start_tick: u64,
        end_tick: u64,
        offset: Option<usize>,
        limit: usize,
        after_sequence: Option<u64>,
        tail_limit: Option<usize>,
    ) -> Result<RecordedAuditPage, RecordingStoreError> {
        let total_events = self.connection.query_row(
            "SELECT COUNT(*) FROM recording_generation_audit_events WHERE run_id = ?1 AND generation = ?2 AND tick BETWEEN ?3 AND ?4",
            params![run_id, generation, start_tick, end_tick],
            |row| row.get::<_, usize>(0),
        )?;
        let tail_start = tail_limit.map_or(0, |tail| total_events.saturating_sub(tail));
        let sequence_start = match after_sequence {
            Some(sequence) => self.connection.query_row(
                "SELECT COUNT(*) FROM recording_generation_audit_events WHERE run_id = ?1 AND generation = ?2 AND tick BETWEEN ?3 AND ?4 AND sequence <= ?5",
                params![run_id, generation, start_tick, end_tick, sequence],
                |row| row.get::<_, usize>(0),
            )?,
            None => tail_start,
        };
        let offset = after_sequence
            .map(|_| sequence_start)
            .unwrap_or_else(|| offset.unwrap_or(tail_start).min(total_events));
        let limit = limit.max(1);
        let mut statement = self.connection.prepare(
            "SELECT sequence, event_json, event_hash FROM recording_generation_audit_events
             WHERE run_id = ?1 AND generation = ?2 AND tick BETWEEN ?3 AND ?4
             ORDER BY sequence ASC LIMIT ?5 OFFSET ?6",
        )?;
        let events = statement
            .query_map(
                params![run_id, generation, start_tick, end_tick, limit, offset],
                |row| {
                    Ok((
                        row.get::<_, u64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?
            .map(|row| {
                let (sequence, event_json, event_hash) = row?;
                if hash_payload(event_json.as_bytes()) != event_hash {
                    return Err(RecordingStoreError::PayloadHashMismatch(event_hash));
                }
                Ok(SequencedRecordedAuditEvent {
                    sequence,
                    event: serde_json::from_str(&event_json)?,
                })
            })
            .collect::<Result<Vec<_>, RecordingStoreError>>()?;
        let end = offset.saturating_add(events.len());
        let has_more = end < total_events;
        Ok(RecordedAuditPage {
            next_offset: (after_sequence.is_none() && has_more).then_some(end),
            next_sequence: has_more.then(|| events.last().expect("page is nonempty").sequence),
            events,
            total_events,
            offset,
            truncated: tail_start > 0,
        })
    }

    fn initialize(&mut self) -> Result<(), RecordingStoreError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS recordings (
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
                open_world_checkpoint_json TEXT NOT NULL DEFAULT 'null',
                active_generation INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS recording_ticks (
                run_id TEXT NOT NULL REFERENCES recordings(run_id) ON DELETE CASCADE,
                tick INTEGER NOT NULL,
                snapshot_hash TEXT NOT NULL,
                payload_hash TEXT NOT NULL,
                payload_size INTEGER NOT NULL,
                PRIMARY KEY(run_id, tick)
             );
             CREATE INDEX IF NOT EXISTS recording_ticks_by_hash
               ON recording_ticks(run_id, snapshot_hash);
             CREATE TABLE IF NOT EXISTS recording_generation_ticks (
                run_id TEXT NOT NULL REFERENCES recordings(run_id) ON DELETE CASCADE,
                generation INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                snapshot_hash TEXT NOT NULL,
                payload_hash TEXT NOT NULL,
                payload_size INTEGER NOT NULL,
                PRIMARY KEY(run_id, generation, tick)
             );
             CREATE INDEX IF NOT EXISTS recording_generation_ticks_by_active
               ON recording_generation_ticks(run_id, generation, tick);
             CREATE TABLE IF NOT EXISTS recording_generation_audit_events (
                run_id TEXT NOT NULL REFERENCES recordings(run_id) ON DELETE CASCADE,
                generation INTEGER NOT NULL,
                sequence INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                event_json TEXT NOT NULL,
                event_hash TEXT NOT NULL,
                PRIMARY KEY(run_id, generation, sequence)
             );
             CREATE INDEX IF NOT EXISTS recording_generation_audit_events_by_window
               ON recording_generation_audit_events(run_id, generation, tick, sequence);",
        )?;
        let has_human_turns = self
            .connection
            .prepare("PRAGMA table_info(recordings)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|name| name == "human_turns_json");
        if !has_human_turns {
            self.connection.execute(
                "ALTER TABLE recordings ADD COLUMN human_turns_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        let has_provenance = self
            .connection
            .prepare("PRAGMA table_info(recordings)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|name| name == "provenance_json");
        if !has_provenance {
            self.connection.execute(
                "ALTER TABLE recordings ADD COLUMN provenance_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }
        let has_open_world_checkpoint = self
            .connection
            .prepare("PRAGMA table_info(recordings)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|name| name == "open_world_checkpoint_json");
        if !has_open_world_checkpoint {
            self.connection.execute(
                "ALTER TABLE recordings ADD COLUMN open_world_checkpoint_json TEXT NOT NULL DEFAULT 'null'",
                [],
            )?;
        }
        let has_active_generation = self
            .connection
            .prepare("PRAGMA table_info(recordings)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|name| name == "active_generation");
        if !has_active_generation {
            self.connection.execute(
                "ALTER TABLE recordings ADD COLUMN active_generation INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        // Existing v0 rows become generation 0. This copy is idempotent and
        // lets upgraded readers use one active-generation query.
        self.connection.execute(
            "INSERT OR IGNORE INTO recording_generation_ticks (run_id, generation, tick, snapshot_hash, payload_hash, payload_size)
             SELECT run_id, 0, tick, snapshot_hash, payload_hash, payload_size FROM recording_ticks",
            [],
        )?;
        self.connection.execute(
            "DELETE FROM recording_generation_ticks
             WHERE NOT EXISTS (
                SELECT 1 FROM recordings
                WHERE recordings.run_id = recording_generation_ticks.run_id
                  AND recordings.active_generation = recording_generation_ticks.generation
             )",
            [],
        )?;
        self.connection.execute(
            "DELETE FROM recording_generation_audit_events
             WHERE NOT EXISTS (
                SELECT 1 FROM recordings
                WHERE recordings.run_id = recording_generation_audit_events.run_id
                  AND recordings.active_generation = recording_generation_audit_events.generation
             )",
            [],
        )?;
        Ok(())
    }
}

fn page_audit_events(
    all_events: Vec<SequencedRecordedAuditEvent>,
    offset: Option<usize>,
    limit: usize,
    after_sequence: Option<u64>,
    tail_limit: Option<usize>,
) -> RecordedAuditPage {
    let total_events = all_events.len();
    let tail_start = tail_limit.map_or(0, |tail| total_events.saturating_sub(tail));
    let sequence_start = after_sequence.map_or(tail_start, |sequence| {
        all_events.partition_point(|event| event.sequence <= sequence)
    });
    let offset = after_sequence
        .map(|_| sequence_start)
        .unwrap_or_else(|| offset.unwrap_or(tail_start).min(total_events));
    let end = offset.saturating_add(limit.max(1)).min(total_events);
    let has_more = end < total_events;
    RecordedAuditPage {
        next_offset: (after_sequence.is_none() && has_more).then_some(end),
        next_sequence: has_more.then(|| all_events[end - 1].sequence),
        events: all_events[offset..end].to_vec(),
        total_events,
        offset,
        truncated: tail_start > 0,
    }
}

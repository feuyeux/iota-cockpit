use std::fs;

use cockpit_recording::PayloadStore;
use serde_json::Value;

#[test]
fn content_addressed_payloads_deduplicate_and_verify_hashes() {
    let root = std::env::temp_dir().join(format!("cockpit-payload-test-{}", uuid::Uuid::new_v4()));
    let store = PayloadStore::new(&root).expect("payload store creates");
    let payload = br#"{"state":"safe"}"#;
    let first = store.put(payload).expect("payload stores");
    let second = store.put(payload).expect("duplicate payload stores");
    assert_eq!(first, second);
    assert_eq!(store.get(&first).expect("payload reads"), payload);
    assert!(store.path_for(&first).exists());
    let files = fs::read_dir(root.join(&first[7..9]))
        .expect("fanout directory reads")
        .count();
    assert_eq!(files, 1);
}

#[test]
fn missing_primary_payload_is_restored_from_the_verified_local_replica() {
    let root =
        std::env::temp_dir().join(format!("cockpit-payload-missing-{}", uuid::Uuid::new_v4()));
    let store = PayloadStore::new(&root).expect("payload store creates");
    let hash = store.put(br#"{"state":"safe"}"#).expect("payload stores");
    fs::remove_file(store.path_for(&hash)).expect("payload deletes");
    assert_eq!(
        store.get(&hash).expect("replica restores payload"),
        br#"{"state":"safe"}"#
    );
    assert!(store.path_for(&hash).exists(), "primary object is restored");
    let replica = PayloadStore::new(root.with_extension("replicas")).expect("replica store");
    fs::remove_file(replica.path_for(&hash)).expect("replica deletes");
    fs::remove_file(store.path_for(&hash)).expect("primary deletes again");
    let error = store
        .get(&hash)
        .expect_err("both copies missing fails explicitly");
    assert!(
        matches!(error, cockpit_recording::RecordingStoreError::PayloadMissing(value) if value == hash)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn authenticated_replica_restores_payload_after_both_local_copies_are_missing() {
    let root = std::env::temp_dir().join(format!("cockpit-payload-auth-{}", uuid::Uuid::new_v4()));
    let auth_root = root.with_extension("external-replica");
    let key = b"0123456789abcdef-auth-key";
    let store = PayloadStore::new(&root)
        .expect("payload store creates")
        .with_authenticated_replica(&auth_root, key)
        .expect("authenticated replica configures");
    let payload = br#"{"state":"remote"}"#;
    let hash = store.put(payload).expect("payload stores");
    fs::remove_file(store.path_for(&hash)).expect("primary deletes");
    let local_replica =
        PayloadStore::new(root.with_extension("replicas")).expect("local replica opens");
    fs::remove_file(local_replica.path_for(&hash)).expect("local replica deletes");

    let (restored, evidence) = store
        .get_with_evidence(&hash)
        .expect("authenticated replica restores");
    assert_eq!(restored, payload);
    assert_eq!(evidence.source, "authenticatedRemoteReplica");
    assert!(evidence.verified);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(auth_root);
}

#[test]
fn authenticated_replica_rejects_tampering() {
    let root =
        std::env::temp_dir().join(format!("cockpit-payload-tamper-{}", uuid::Uuid::new_v4()));
    let auth_root = root.with_extension("external-replica");
    let store = PayloadStore::new(&root)
        .expect("payload store creates")
        .with_authenticated_replica(&auth_root, b"0123456789abcdef-auth-key")
        .expect("authenticated replica configures");
    let hash = store.put(br#"{"state":"signed"}"#).expect("payload stores");
    fs::remove_file(store.path_for(&hash)).expect("primary deletes");
    let local_replica =
        PayloadStore::new(root.with_extension("replicas")).expect("local replica opens");
    fs::remove_file(local_replica.path_for(&hash)).expect("local replica deletes");
    let replica_path = auth_root
        .join(&hash[7..9])
        .join(format!("{}.replica", &hash[7..]));
    let mut envelope: Value =
        serde_json::from_slice(&fs::read(&replica_path).expect("replica reads"))
            .expect("replica envelope parses");
    let mac = envelope
        .get("macBase64")
        .and_then(Value::as_str)
        .expect("replica has mac")
        .to_string();
    let replacement = if mac.starts_with('A') { "B" } else { "A" };
    envelope["macBase64"] = Value::String(format!("{replacement}{}", &mac[1..]));
    fs::write(
        &replica_path,
        serde_json::to_vec(&envelope).expect("tampered envelope serializes"),
    )
    .expect("tamper writes");
    assert!(
        matches!(store.get(&hash), Err(cockpit_recording::RecordingStoreError::PayloadHashMismatch(value)) if value == hash)
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(auth_root);
}

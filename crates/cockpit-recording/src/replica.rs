//! Authenticated content-addressed replica storage.
//!
//! The replica root is intentionally configured separately from the primary
//! payload root. Deployments may place it on a mounted remote/object-backed
//! filesystem; the envelope MAC prevents an untrusted location from being
//! accepted merely because its filename matches a payload hash.

use std::{fs, io::Write, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::hmac;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::RecordingStoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplicaEnvelope {
    hash: String,
    payload_base64: String,
    mac_base64: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedReplicaStore {
    root: PathBuf,
    key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadRestoreEvidence {
    pub hash: String,
    pub source: String,
    pub verified: bool,
}

impl AuthenticatedReplicaStore {
    pub fn new(
        root: impl Into<PathBuf>,
        key: impl AsRef<[u8]>,
    ) -> Result<Self, RecordingStoreError> {
        let key = key.as_ref().to_vec();
        if key.len() < 16 {
            return Err(RecordingStoreError::InvalidReplicaAuth);
        }
        let store = Self {
            root: root.into(),
            key,
        };
        fs::create_dir_all(&store.root)
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        Ok(store)
    }

    pub fn put(&self, hash: &str, payload: &[u8]) -> Result<(), RecordingStoreError> {
        if hash_payload(payload) != hash {
            return Err(RecordingStoreError::PayloadHashMismatch(hash.to_string()));
        }
        let encoded = STANDARD.encode(payload);
        let mac = self.mac(hash, &encoded);
        let envelope = ReplicaEnvelope {
            hash: hash.to_string(),
            payload_base64: encoded,
            mac_base64: STANDARD.encode(mac),
        };
        self.publish(self.path_for(hash), &serde_json::to_vec(&envelope)?)
    }

    pub fn get(
        &self,
        hash: &str,
    ) -> Result<(Vec<u8>, PayloadRestoreEvidence), RecordingStoreError> {
        let path = self.path_for(hash);
        let bytes = fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RecordingStoreError::PayloadMissing(hash.to_string())
            } else {
                RecordingStoreError::Io(error.to_string())
            }
        })?;
        let envelope: ReplicaEnvelope = serde_json::from_slice(&bytes)?;
        if envelope.hash != hash {
            return Err(RecordingStoreError::PayloadHashMismatch(hash.to_string()));
        }
        let expected = self.mac(hash, &envelope.payload_base64);
        let provided = STANDARD
            .decode(envelope.mac_base64)
            .map_err(|_| RecordingStoreError::PayloadHashMismatch(hash.to_string()))?;
        hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, &self.key),
            &self.mac_input(hash, &envelope.payload_base64),
            &provided,
        )
        .map_err(|_| RecordingStoreError::PayloadHashMismatch(hash.to_string()))?;
        let payload = STANDARD
            .decode(envelope.payload_base64)
            .map_err(|_| RecordingStoreError::PayloadHashMismatch(hash.to_string()))?;
        if hash_payload(&payload) != hash || expected != provided {
            return Err(RecordingStoreError::PayloadHashMismatch(hash.to_string()));
        }
        Ok((
            payload,
            PayloadRestoreEvidence {
                hash: hash.to_string(),
                source: "authenticatedRemoteReplica".to_string(),
                verified: true,
            },
        ))
    }

    fn path_for(&self, hash: &str) -> PathBuf {
        let digest = hash.strip_prefix("sha256:").unwrap_or(hash);
        self.root
            .join(digest.get(..2).unwrap_or("00"))
            .join(format!("{digest}.replica"))
    }

    fn mac(&self, hash: &str, encoded: &str) -> Vec<u8> {
        hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, &self.key),
            &self.mac_input(hash, encoded),
        )
        .as_ref()
        .to_vec()
    }

    fn mac_input(&self, hash: &str, encoded: &str) -> Vec<u8> {
        let mut input = Vec::with_capacity(hash.len() + encoded.len() + 1);
        input.extend_from_slice(hash.as_bytes());
        input.push(0);
        input.extend_from_slice(encoded.as_bytes());
        input
    }

    fn publish(&self, path: PathBuf, bytes: &[u8]) -> Result<(), RecordingStoreError> {
        fs::create_dir_all(path.parent().expect("replica parent"))
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        let temp = path.with_file_name(format!(".{}.{}.tmp", uuid::Uuid::new_v4(), "replica"));
        let mut file =
            fs::File::create(&temp).map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        file.sync_all()
            .map_err(|error| RecordingStoreError::Io(error.to_string()))?;
        fs::rename(&temp, path).map_err(|error| RecordingStoreError::Io(error.to_string()))
    }
}

fn hash_payload(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("sha256:{:x}", hasher.finalize())
}

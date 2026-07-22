use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::{
    rand::SystemRandom,
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};

use crate::RulePolicy;

pub const RULE_POLICY_BUNDLE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct RulePolicyBundle {
    policies: BTreeMap<String, RulePolicy>,
    revoked_policies: std::collections::BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: u32,
    policies: BTreeMap<String, ManifestPolicy>,
    #[serde(default)]
    revoked_policies: std::collections::BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPolicy {
    file: String,
    hash: String,
}

impl RulePolicyBundle {
    pub fn discover_base64(directory: impl AsRef<Path>, trust_root: &str) -> Result<Self, String> {
        let roots = trust_root
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                let key = STANDARD
                    .decode(value)
                    .map_err(|error| format!("invalid policy trust-root base64: {error}"))?;
                key.try_into().map_err(|_| {
                    "policy trust root must be a 32-byte Ed25519 public key".to_string()
                })
            })
            .collect::<Result<Vec<[u8; 32]>, String>>()?;
        Self::discover_with_trust_roots(directory, &roots)
    }

    pub fn discover(directory: impl AsRef<Path>, trust_root: &[u8; 32]) -> Result<Self, String> {
        Self::discover_with_trust_roots(directory, std::slice::from_ref(trust_root))
    }

    pub fn discover_with_trust_roots(
        directory: impl AsRef<Path>,
        trust_roots: &[[u8; 32]],
    ) -> Result<Self, String> {
        if trust_roots.is_empty() {
            return Err("at least one policy trust root is required".to_string());
        }
        let directory = directory.as_ref();
        let manifest_path = directory.join("manifest.json");
        let manifest_bytes = std::fs::read(&manifest_path).map_err(|error| {
            format!(
                "failed to read policy manifest {}: {error}",
                manifest_path.display()
            )
        })?;
        let signature_path = directory.join("manifest.sig");
        let signature_text = std::fs::read_to_string(&signature_path).map_err(|error| {
            format!(
                "failed to read policy signature {}: {error}",
                signature_path.display()
            )
        })?;
        let signature = STANDARD
            .decode(signature_text.trim())
            .map_err(|error| format!("invalid policy manifest base64 signature: {error}"))?;
        if !trust_roots.iter().any(|trust_root| {
            UnparsedPublicKey::new(&ED25519, trust_root)
                .verify(&manifest_bytes, &signature)
                .is_ok()
        }) {
            return Err("policy manifest signature verification failed".to_string());
        }
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("invalid policy manifest: {error}"))?;
        if manifest.version != RULE_POLICY_BUNDLE_VERSION {
            return Err(format!(
                "unsupported policy bundle version {}",
                manifest.version
            ));
        }
        let mut policies = BTreeMap::new();
        for (id, entry) in manifest.policies {
            if id.trim().is_empty() || !is_plain_file_name(&entry.file) {
                return Err(
                    "policy manifest contains an invalid policy id or file path".to_string()
                );
            }
            let path = directory.join(&entry.file);
            let policy = RulePolicy::from_file(&path)?;
            if policy.hash() != entry.hash {
                return Err(format!(
                    "policy '{id}' hash does not match its signed manifest"
                ));
            }
            policies.insert(id, policy);
        }
        let revoked_policies = manifest.revoked_policies;
        if revoked_policies.iter().any(|id| !policies.contains_key(id)) {
            return Err("policy manifest revokes an unknown policy id".to_string());
        }
        Ok(Self {
            policies,
            revoked_policies,
        })
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.policies
            .keys()
            .filter(|id| !self.revoked_policies.contains(*id))
            .map(String::as_str)
    }

    pub fn select(&self, id: &str) -> Result<RulePolicy, String> {
        if self.revoked_policies.contains(id) {
            return Err(format!("policy '{id}' has been revoked"));
        }
        self.policies
            .get(id)
            .cloned()
            .ok_or_else(|| format!("policy '{id}' is not in the trusted bundle"))
    }

    /// Generate a PKCS#8 Ed25519 private key and its base64 public trust root.
    /// The private material is returned only to the caller so a CLI can write
    /// it with restrictive filesystem permissions.
    pub fn generate_signing_key() -> Result<(String, String), String> {
        let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|error| format!("failed to generate Ed25519 key: {error}"))?;
        let pair = Ed25519KeyPair::from_pkcs8(key.as_ref())
            .map_err(|error| format!("generated Ed25519 key is invalid: {error}"))?;
        Ok((
            STANDARD.encode(key.as_ref()),
            STANDARD.encode(pair.public_key().as_ref()),
        ))
    }

    /// Sign the exact manifest bytes consumed by `discover`.
    pub fn sign_manifest(private_key_base64: &str, manifest: &[u8]) -> Result<String, String> {
        let private_key = STANDARD
            .decode(private_key_base64.trim())
            .map_err(|error| format!("invalid Ed25519 private key base64: {error}"))?;
        let pair = Ed25519KeyPair::from_pkcs8(&private_key)
            .map_err(|error| format!("invalid Ed25519 private key: {error}"))?;
        Ok(STANDARD.encode(pair.sign(manifest).as_ref()))
    }

    pub fn public_key_from_private(private_key_base64: &str) -> Result<String, String> {
        let private_key = STANDARD
            .decode(private_key_base64.trim())
            .map_err(|error| format!("invalid Ed25519 private key base64: {error}"))?;
        let pair = Ed25519KeyPair::from_pkcs8(&private_key)
            .map_err(|error| format!("invalid Ed25519 private key: {error}"))?;
        Ok(STANDARD.encode(pair.public_key().as_ref()))
    }
}

fn is_plain_file_name(value: &str) -> bool {
    let path = PathBuf::from(value);
    path.extension()
        .is_some_and(|extension| extension == "json")
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };

    use super::{Manifest, ManifestPolicy, RULE_POLICY_BUNDLE_VERSION, RulePolicyBundle};
    use crate::RulePolicy;

    #[test]
    fn discovers_only_signed_hashed_policies() {
        let directory =
            std::env::temp_dir().join(format!("cockpit-policy-bundle-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("directory creates");
        let policy = RulePolicy::default();
        fs::write(
            directory.join("baseline.json"),
            serde_json::to_vec(&policy).expect("policy serializes"),
        )
        .expect("policy writes");
        let manifest = Manifest {
            version: RULE_POLICY_BUNDLE_VERSION,
            policies: [(
                "baseline".to_string(),
                ManifestPolicy {
                    file: "baseline.json".to_string(),
                    hash: policy.hash(),
                },
            )]
            .into_iter()
            .collect(),
            revoked_policies: Default::default(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest serializes");
        let key = Ed25519KeyPair::from_pkcs8(
            Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .expect("key generates")
                .as_ref(),
        )
        .expect("key parses");
        fs::write(directory.join("manifest.json"), &manifest_bytes).expect("manifest writes");
        fs::write(
            directory.join("manifest.sig"),
            STANDARD.encode(key.sign(&manifest_bytes).as_ref()),
        )
        .expect("signature writes");
        let public_key: [u8; 32] = key
            .public_key()
            .as_ref()
            .try_into()
            .expect("ed25519 public key length");
        let bundle =
            RulePolicyBundle::discover(&directory, &public_key).expect("signed bundle discovers");
        assert_eq!(bundle.select("baseline").expect("policy selects"), policy);
        assert!(bundle.select("unknown").is_err());
        fs::write(directory.join("baseline.json"), b"{}").expect("policy tampers");
        assert!(RulePolicyBundle::discover(&directory, &public_key).is_err());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn generated_signing_key_and_revoked_policy_are_enforced() {
        let directory = std::env::temp_dir().join(format!(
            "cockpit-policy-revocation-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).expect("directory creates");
        let policy = RulePolicy::default();
        fs::write(
            directory.join("baseline.json"),
            serde_json::to_vec(&policy).expect("policy serializes"),
        )
        .expect("policy writes");
        let manifest = serde_json::json!({
            "version": RULE_POLICY_BUNDLE_VERSION,
            "policies": {
                "baseline": { "file": "baseline.json", "hash": policy.hash() }
            },
            "revokedPolicies": ["baseline"]
        });
        let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest serializes");
        let (private, public) = RulePolicyBundle::generate_signing_key().expect("key generates");
        fs::write(directory.join("manifest.json"), &manifest_bytes).expect("manifest writes");
        fs::write(
            directory.join("manifest.sig"),
            RulePolicyBundle::sign_manifest(&private, &manifest_bytes).expect("signs"),
        )
        .expect("signature writes");
        let bundle = RulePolicyBundle::discover_base64(&directory, &public)
            .expect("revoked bundle discovers");
        assert_eq!(bundle.ids().count(), 0);
        assert!(bundle.select("baseline").is_err());
        assert_eq!(
            RulePolicyBundle::public_key_from_private(&private).expect("public key derives"),
            public
        );
        let _ = fs::remove_dir_all(directory);
    }
}

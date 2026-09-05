//! Regression tests for public security helpers using only synthetic data.

use async_trait::async_trait;
use rustible::audit::hashchain::{HashChainEntry, HashChainState};
use rustible::audit::immutable::{
    AuditStorage, FileAuditStorage, ImmutableAuditError, ImmutableAuditResult, ImmutableAuditStore,
};
use rustible::audit::verify::AuditVerifier;
use rustible::security::password_cache::PasswordCache;
use rustible::security::path::validate_path_within_base;
use rustible::security::signing::keys::SigningKeyPair;
use rustible::security::signing::signer::ArtifactSigner;
use rustible::security::signing::trust::TrustPolicy;
use rustible::security::signing::verifier::{ArtifactVerifier, TrustLevel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

#[test]
fn containment_accepts_existing_relative_base_and_missing_child() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let relative = std::path::PathBuf::from(dir.path().file_name().unwrap());
    let expected = dir.path().canonicalize().unwrap().join("new/file");
    assert_eq!(
        validate_path_within_base(&relative, "new/file").unwrap(),
        expected
    );
}

#[test]
fn containment_resolves_a_missing_base_from_existing_ancestor() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("new/base");
    assert_eq!(
        validate_path_within_base(&base, "child").unwrap(),
        dir.path().canonicalize().unwrap().join("new/base/child")
    );
}

#[cfg(unix)]
#[test]
fn containment_rejects_existing_and_missing_targets_through_outside_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&base).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("existing"), b"fixture").unwrap();
    symlink(&outside, base.join("link")).unwrap();
    assert!(validate_path_within_base(&base, "link/existing").is_err());
    assert!(validate_path_within_base(&base, "link/missing/child").is_err());
}

#[cfg(unix)]
#[test]
fn containment_resolves_inside_links_and_rejects_dangling_links() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    std::fs::create_dir(base.join("inside")).unwrap();
    symlink("inside", base.join("link")).unwrap();
    assert_eq!(
        validate_path_within_base(&base, "link/missing").unwrap(),
        base.join("inside/missing")
    );
    symlink("absent", base.join("dangling")).unwrap();
    assert!(validate_path_within_base(&base, "dangling/child").is_err());
}

#[test]
fn verification_cannot_substitute_a_trusted_bundle_identity() {
    // Fixed bytes are a public test fixture, never an issued credential.
    let key = SigningKeyPair::from_bytes("fixture-untrusted", &[7; 32]).unwrap();
    let data = b"synthetic artifact";
    let mut bundle = ArtifactSigner::new().sign_bytes(data, &key);
    bundle.key_id = "fixture-trusted".into();
    let mut policy = TrustPolicy::default();
    policy.trusted_keys.insert(bundle.key_id.clone());
    let result = ArtifactVerifier::new().verify_with_policy(data, &bundle, &key, Some(&policy));
    assert!(!result.valid);
    assert_eq!(result.trust_level, TrustLevel::Unknown);
    assert_eq!(result.key_id, key.id);
}

#[test]
fn verification_preserves_unknown_and_revoked_key_contracts() {
    let key = SigningKeyPair::from_bytes("fixture-key", &[9; 32]).unwrap();
    let data = b"synthetic artifact";
    let bundle = ArtifactSigner::new().sign_bytes(data, &key);
    let verifier = ArtifactVerifier::new();
    let unknown = verifier.verify(data, &bundle, &key);
    assert!(unknown.valid);
    assert_eq!(unknown.trust_level, TrustLevel::Unknown);
    let mut policy = TrustPolicy::default();
    policy.trusted_keys.insert(key.id.clone());
    policy.revoked_keys.insert(key.id.clone());
    let revoked = verifier.verify_with_policy(data, &bundle, &key, Some(&policy));
    assert!(revoked.valid);
    assert_eq!(revoked.trust_level, TrustLevel::Revoked);
}

#[derive(Clone, Default)]
struct SyntheticStorage {
    entries: Arc<Mutex<Vec<HashChainEntry>>>,
    fail_next: Arc<AtomicBool>,
}

#[async_trait]
impl AuditStorage for SyntheticStorage {
    async fn append(&self, entry: &HashChainEntry) -> ImmutableAuditResult<()> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(std::io::Error::other("synthetic append failure").into());
        }
        self.entries.lock().unwrap().push(entry.clone());
        Ok(())
    }

    async fn read_all(&self) -> ImmutableAuditResult<Vec<HashChainEntry>> {
        Ok(self.entries.lock().unwrap().clone())
    }
}

#[tokio::test]
async fn failed_append_does_not_advance_state_or_break_next_record() {
    let storage = SyntheticStorage::default();
    let mut store = ImmutableAuditStore::new(Box::new(storage.clone()));
    store.record(b"first").await.unwrap();
    storage.fail_next.store(true, Ordering::SeqCst);
    assert!(store.record(b"failed").await.is_err());
    assert_eq!(store.next_sequence(), 1);
    assert_eq!(store.record(b"second").await.unwrap().sequence, 1);
    assert!(store.verify().await.unwrap());
    let mut reopened = ImmutableAuditStore::open(Box::new(storage)).await.unwrap();
    assert_eq!(reopened.record(b"third").await.unwrap().sequence, 2);
}

#[tokio::test]
async fn reopen_rejects_tampered_but_parseable_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut chain = HashChainState::new();
    let mut entry = chain.append(b"synthetic event");
    entry.event_hash = "modified".into();
    std::fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
    assert!(matches!(
        ImmutableAuditStore::open(Box::new(FileAuditStorage::new(path))).await,
        Err(ImmutableAuditError::VerificationFailed)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn reopen_read_error_is_not_treated_as_an_empty_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop");
    std::os::unix::fs::symlink("loop", &path).unwrap();
    assert!(matches!(
        ImmutableAuditStore::open(Box::new(FileAuditStorage::new(path))).await,
        Err(ImmutableAuditError::Io(_))
    ));
}

#[tokio::test]
async fn reopen_read_missing_file_starts_a_valid_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new-audit.jsonl");
    let mut store = ImmutableAuditStore::open(Box::new(FileAuditStorage::new(path)))
        .await
        .unwrap();
    assert_eq!(store.next_sequence(), 0);
    assert_eq!(
        store.record(b"first fixture event").await.unwrap().sequence,
        0
    );
    assert!(store.verify().await.unwrap());
}

fn boundary_entry(sequence: u64, previous_hash: String) -> HashChainEntry {
    let event_hash = blake3::hash(b"synthetic boundary event")
        .to_hex()
        .to_string();
    let mut input = sequence.to_le_bytes().to_vec();
    input.extend_from_slice(event_hash.as_bytes());
    input.extend_from_slice(previous_hash.as_bytes());
    HashChainEntry {
        sequence,
        timestamp: "fixture".into(),
        event_hash,
        previous_hash,
        chain_hash: blake3::hash(&input).to_hex().to_string(),
    }
}

#[tokio::test]
async fn exhausted_audit_sequence_returns_error_without_appending() {
    let storage = SyntheticStorage::default();
    storage
        .entries
        .lock()
        .unwrap()
        .push(boundary_entry(u64::MAX - 1, String::new()));
    let mut store = ImmutableAuditStore::open(Box::new(storage.clone()))
        .await
        .unwrap();
    assert!(store.record(b"cannot append").await.is_err());
    assert_eq!(store.next_sequence(), u64::MAX);
    assert_eq!(storage.entries.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn reopen_exhausted_sequence_returns_error_without_panicking() {
    let storage = SyntheticStorage::default();
    storage
        .entries
        .lock()
        .unwrap()
        .push(boundary_entry(u64::MAX, String::new()));
    assert!(ImmutableAuditStore::open(Box::new(storage)).await.is_err());
}

#[test]
fn sequence_wrap_is_rejected_by_both_verifiers() {
    let first = boundary_entry(u64::MAX, String::new());
    let second = boundary_entry(0, first.chain_hash.clone());
    let entries = [first, second];
    assert!(!HashChainState::verify_chain(&entries));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let content = entries
        .iter()
        .map(|entry| serde_json::to_string(entry).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, content).unwrap();
    assert!(!AuditVerifier::verify_file(&path).unwrap().valid);
}

#[test]
fn high_security_cache_allows_only_one_successful_reader() {
    let cache = Arc::new(PasswordCache::high_security());
    cache.store("fixture-host", "fixture-user", "public test value");
    let barrier = Arc::new(Barrier::new(8));
    let readers = (0..8)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                cache.get("fixture-host", "fixture-user").is_ok()
            })
        })
        .collect::<Vec<_>>();
    let successes = readers
        .into_iter()
        .map(|r| usize::from(r.join().unwrap()))
        .sum::<usize>();
    assert_eq!(successes, 1);
    assert!(!cache.has("fixture-host", "fixture-user"));
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().misses, 7);
}

#[test]
fn ordinary_cache_retains_value_and_tracks_uses() {
    let cache = PasswordCache::new();
    cache.store("fixture-host", "fixture-user", "public test value");
    assert!(cache.get("fixture-host", "fixture-user").is_ok());
    assert!(cache.get("fixture-host", "fixture-user").is_ok());
    assert!(cache.has("fixture-host", "fixture-user"));
    assert_eq!(cache.entries_info()[0].use_count, 2);
    assert_eq!(cache.stats().hits, 2);
    assert_eq!(cache.stats().misses, 0);
}

#[test]
fn expired_one_time_value_counts_one_expiration_and_no_hit() {
    let cache = PasswordCache::high_security();
    cache.store_with_ttl(
        "fixture-host",
        "fixture-user",
        "public test value",
        Duration::ZERO,
    );
    assert!(cache.get("fixture-host", "fixture-user").is_err());
    assert!(cache.is_empty());
    assert_eq!(cache.stats().expirations, 1);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(cache.stats().misses, 1);
}

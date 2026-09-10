//! Hash and checksum filters for Jinja2 templates.
//!
//! This module provides cryptographic hash and checksum filters that are
//! compatible with Ansible's Jinja2 hash filters.
//!
//! # Available Filters
//!
//! - `hash`: Compute hash of a string (supports multiple algorithms)
//! - `checksum`: Compute checksum of a string (alias for sha1 hash)
//! - `password_hash`: Generate password hash (for /etc/shadow format)
//! - `md5`: Compute MD5 hash
//! - `sha1`: Compute SHA-1 hash
//! - `sha256`: Compute SHA-256 hash
//! - `sha512`: Compute SHA-512 hash
//!
//! # Supported Algorithms
//!
//! The `hash` filter supports the following algorithms:
//! - `md5`
//! - `sha1`
//! - `sha1` (default, matching Ansible)
//! - `sha384`
//! - `sha512`
//!
//! # Examples
//!
//! ```jinja2
//! {{ 'secret' | hash('sha256') }}
//! {{ 'password' | password_hash('sha512') }}
//! {{ data | checksum }}
//! {{ 'hello' | md5 }}
//! ```

use minijinja::{Environment, Error, ErrorKind, Value};
use sha2::Digest;

/// Register all hash filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("hash", hash);
    env.add_filter("checksum", checksum);
    env.add_filter("password_hash", password_hash);
    env.add_filter("md5", md5_filter);
    env.add_filter("sha1", sha1_filter);
    env.add_filter("sha256", sha256_filter);
    env.add_filter("sha512", sha512_filter);
}

/// Compute hash of a string using the specified algorithm.
///
/// # Arguments
///
/// * `input` - The string to hash
/// * `algorithm` - The hash algorithm to use (default: "sha256")
///
/// # Returns
///
/// The hexadecimal string representation of the hash.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `hash` filter, which defaults to sha1. Supports
/// md5, sha1, sha256, sha384 and sha512; any other algorithm is an error
/// rather than a silent substitution.
fn hash(input: Value, algorithm: Option<String>) -> Result<String, Error> {
    let data = value_to_string(&input);
    let algo = algorithm.unwrap_or_else(|| "sha1".to_string());

    match algo.to_lowercase().as_str() {
        "md5" => Ok(compute_md5(&data)),
        "sha1" => Ok(compute_sha1(&data)),
        "sha256" => Ok(compute_sha256(&data)),
        "sha384" => Ok(compute_sha384(&data)),
        "sha512" => Ok(compute_sha512(&data)),
        other => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "hash: unsupported algorithm '{}' (supported: md5, sha1, sha256, sha384, sha512)",
                other
            ),
        )),
    }
}

/// Compute checksum of a string (SHA-1).
///
/// # Arguments
///
/// * `input` - The string to compute checksum for
///
/// # Returns
///
/// The hexadecimal SHA-1 checksum.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `checksum` filter, which uses SHA-1.
fn checksum(input: Value) -> String {
    let data = value_to_string(&input);
    compute_sha1(&data)
}

/// Generate a crypt(3)-compatible password hash suitable for `/etc/shadow`.
///
/// # Arguments
///
/// * `password` - The password to hash
/// * `hashtype` - The hash scheme: `sha512` (default) or `sha256`
/// * `salt` - Optional salt (a random 16-character salt is generated if omitted)
/// * `rounds` - Optional round count; omitted means the crypt default of 5000
///
/// # Returns
///
/// A crypt-style password hash string (`$6$...` for SHA-512, `$5$...` for
/// SHA-256), or an error for an unsupported scheme.
///
/// # Ansible Compatibility
///
/// Implements the SHA-crypt schemes used by Ansible's `password_hash` filter.
/// Schemes that Ansible reaches through passlib but Rustible does not
/// implement (`md5_crypt`, `bcrypt`, `des_crypt`, ...) fail with an explicit
/// error instead of returning a hash the target system cannot verify.
fn password_hash(
    password: String,
    hashtype: Option<String>,
    salt: Option<String>,
    rounds: Option<u32>,
) -> Result<String, Error> {
    let hashtype = hashtype.unwrap_or_else(|| "sha512".to_string());
    let salt = match salt {
        Some(salt) => validate_salt(&salt)?,
        None => generate_salt(),
    };
    let rounds = rounds.map(|rounds| rounds.clamp(sha_crypt::MIN_ROUNDS, sha_crypt::MAX_ROUNDS));

    match hashtype.to_lowercase().as_str() {
        "sha512" | "sha512_crypt" => {
            Ok(sha_crypt::sha512_crypt(password.as_bytes(), &salt, rounds))
        }
        "sha256" | "sha256_crypt" => {
            Ok(sha_crypt::sha256_crypt(password.as_bytes(), &salt, rounds))
        }
        other => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "password_hash: unsupported hash type '{}' (supported: sha512, sha256)",
                other
            ),
        )),
    }
}

/// Validate a caller-supplied salt against crypt(3) salt rules.
///
/// crypt(3) reads at most 16 salt characters and treats `$` as a field
/// separator, so a salt containing one would silently produce a different
/// hash than the caller asked for.
fn validate_salt(salt: &str) -> Result<String, Error> {
    if let Some(bad) = salt
        .chars()
        .find(|c| !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '/'))
    {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "password_hash: invalid salt character '{}' (allowed: A-Z a-z 0-9 . /)",
                bad
            ),
        ));
    }
    Ok(salt.chars().take(sha_crypt::MAX_SALT_LEN).collect())
}

/// Compute MD5 hash.
fn md5_filter(input: Value) -> String {
    let data = value_to_string(&input);
    compute_md5(&data)
}

/// Compute SHA-1 hash.
fn sha1_filter(input: Value) -> String {
    let data = value_to_string(&input);
    compute_sha1(&data)
}

/// Compute SHA-256 hash.
fn sha256_filter(input: Value) -> String {
    let data = value_to_string(&input);
    compute_sha256(&data)
}

/// Compute SHA-512 hash.
fn sha512_filter(input: Value) -> String {
    let data = value_to_string(&input);
    compute_sha512(&data)
}

// ============================================================================
// Helper Functions
// ============================================================================

fn value_to_string(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

fn compute_md5(data: &str) -> String {
    let digest = md5::compute(data.as_bytes());
    hex::encode(digest.0)
}

fn compute_sha1(data: &str) -> String {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

fn compute_sha256(data: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

fn compute_sha384(data: &str) -> String {
    let mut hasher = sha2::Sha384::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

fn compute_sha512(data: &str) -> String {
    let mut hasher = sha2::Sha512::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

fn generate_salt() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789./";
    let mut rng = rand::rng();
    (0..16)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// We need hex encoding, let's add a simple implementation
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

/// SHA-crypt (`$5$` / `$6$`) password hashing.
///
/// Implements Ulrich Drepper's SHA-crypt specification, the scheme used by
/// glibc's crypt(3) for `$5$` (SHA-256) and `$6$` (SHA-512) hashes. Output is
/// byte-for-byte identical to `crypt(3)` and `openssl passwd -5/-6`.
mod sha_crypt {
    use sha2::{Digest, Sha256, Sha512};

    /// Default round count when no `rounds=` prefix is present.
    const DEFAULT_ROUNDS: u32 = 5_000;
    /// Lowest round count crypt(3) accepts.
    pub const MIN_ROUNDS: u32 = 1_000;
    /// Highest round count crypt(3) accepts.
    pub const MAX_ROUNDS: u32 = 999_999_999;
    /// crypt(3) reads at most 16 salt characters.
    pub const MAX_SALT_LEN: usize = 16;

    const B64_ALPHABET: &[u8; 64] =
        b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    /// Byte order used to base64-encode a SHA-512 digest.
    const SHA512_ORDER: [[usize; 3]; 21] = [
        [0, 21, 42],
        [22, 43, 1],
        [44, 2, 23],
        [3, 24, 45],
        [25, 46, 4],
        [47, 5, 26],
        [6, 27, 48],
        [28, 49, 7],
        [50, 8, 29],
        [9, 30, 51],
        [31, 52, 10],
        [53, 11, 32],
        [12, 33, 54],
        [34, 55, 13],
        [56, 14, 35],
        [15, 36, 57],
        [37, 58, 16],
        [59, 17, 38],
        [18, 39, 60],
        [40, 61, 19],
        [62, 20, 41],
    ];

    /// Byte order used to base64-encode a SHA-256 digest.
    const SHA256_ORDER: [[usize; 3]; 10] = [
        [0, 10, 20],
        [21, 1, 11],
        [12, 22, 2],
        [3, 13, 23],
        [24, 4, 14],
        [15, 25, 5],
        [6, 16, 26],
        [27, 7, 17],
        [18, 28, 8],
        [9, 19, 29],
    ];

    /// Hash `password` with SHA-512 crypt, returning a `$6$...` string.
    pub fn sha512_crypt(password: &[u8], salt: &str, rounds: Option<u32>) -> String {
        let digest =
            sha_crypt::<Sha512>(password, salt.as_bytes(), rounds.unwrap_or(DEFAULT_ROUNDS));
        // SHA-512 leaves one digest byte over, encoded as two characters.
        let encoded = encode(&digest, &SHA512_ORDER, |d| (0, 0, d[63]), 2);
        format_hash("6", salt, rounds, &encoded)
    }

    /// Hash `password` with SHA-256 crypt, returning a `$5$...` string.
    pub fn sha256_crypt(password: &[u8], salt: &str, rounds: Option<u32>) -> String {
        let digest =
            sha_crypt::<Sha256>(password, salt.as_bytes(), rounds.unwrap_or(DEFAULT_ROUNDS));
        // SHA-256 leaves two digest bytes over, encoded as three characters.
        let encoded = encode(&digest, &SHA256_ORDER, |d| (0, d[31], d[30]), 3);
        format_hash("5", salt, rounds, &encoded)
    }

    fn format_hash(id: &str, salt: &str, rounds: Option<u32>, encoded: &str) -> String {
        match rounds {
            Some(rounds) => format!("${}$rounds={}${}${}", id, rounds, salt, encoded),
            None => format!("${}${}${}", id, salt, encoded),
        }
    }

    /// The SHA-crypt digest loop, shared by the SHA-256 and SHA-512 variants.
    fn sha_crypt<D: Digest>(password: &[u8], salt: &[u8], rounds: u32) -> Vec<u8> {
        let digest_len = <D as Digest>::output_size();

        // Digest B: password, salt, password.
        let mut b = D::new();
        b.update(password);
        b.update(salt);
        b.update(password);
        let b = b.finalize();

        // Digest A: password, salt, |password| bytes of B, then one pass per
        // bit of the password length adding either B or the password.
        let mut a = D::new();
        a.update(password);
        a.update(salt);
        a.update(cycled(&b, password.len()));
        let mut count = password.len();
        while count > 0 {
            if count & 1 != 0 {
                a.update(&b[..]);
            } else {
                a.update(password);
            }
            count >>= 1;
        }
        let a = a.finalize();

        // Sequence P: |password| bytes derived from the password alone.
        let mut dp = D::new();
        for _ in 0..password.len() {
            dp.update(password);
        }
        let p = cycled(&dp.finalize(), password.len());

        // Sequence S: |salt| bytes derived from the salt alone.
        let mut ds = D::new();
        for _ in 0..(16 + a[0] as usize) {
            ds.update(salt);
        }
        let s = cycled(&ds.finalize(), salt.len());

        let mut current = a.to_vec();
        for round in 0..rounds as usize {
            let mut c = D::new();
            if round & 1 != 0 {
                c.update(&p);
            } else {
                c.update(&current);
            }
            if round % 3 != 0 {
                c.update(&s);
            }
            if round % 7 != 0 {
                c.update(&p);
            }
            if round & 1 != 0 {
                c.update(&current);
            } else {
                c.update(&p);
            }
            current = c.finalize().to_vec();
        }

        debug_assert_eq!(current.len(), digest_len);
        current
    }

    /// Repeat `digest` until `len` bytes are produced.
    fn cycled(digest: &[u8], len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() + digest.len() <= len {
            out.extend_from_slice(digest);
        }
        let remainder = len - out.len();
        out.extend_from_slice(&digest[..remainder]);
        out
    }

    /// Encode a digest using crypt(3)'s permuted base64 order.
    fn encode<const N: usize>(
        digest: &[u8],
        order: &[[usize; 3]; N],
        tail: fn(&[u8]) -> (u8, u8, u8),
        tail_chars: usize,
    ) -> String {
        let mut out = String::with_capacity(N * 4 + tail_chars);
        for group in order {
            push_b64(
                &mut out,
                digest[group[0]],
                digest[group[1]],
                digest[group[2]],
                4,
            );
        }
        let (b2, b1, b0) = tail(digest);
        push_b64(&mut out, b2, b1, b0, tail_chars);
        out
    }

    fn push_b64(out: &mut String, b2: u8, b1: u8, b0: u8, chars: usize) {
        let mut w = ((b2 as u32) << 16) | ((b1 as u32) << 8) | b0 as u32;
        for _ in 0..chars {
            out.push(B64_ALPHABET[(w & 0x3f) as usize] as char);
            w >>= 6;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5() {
        let result = md5_filter(Value::from("hello"));
        // Known MD5 of "hello"
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_sha1() {
        let result = sha1_filter(Value::from("hello"));
        // Known SHA-1 of "hello"
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_sha256() {
        let result = sha256_filter(Value::from("hello"));
        // Known SHA-256 of "hello"
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha512() {
        let result = sha512_filter(Value::from("hello"));
        // Check it's 128 hex characters (512 bits)
        assert_eq!(result.len(), 128);
    }

    #[test]
    fn test_hash_with_algorithm() {
        let result = hash(Value::from("test"), Some("md5".to_string())).unwrap();
        assert_eq!(result, "098f6bcd4621d373cade4e832627b4f6");
    }

    #[test]
    fn test_hash_default_algorithm() {
        let result = hash(Value::from("hello"), None).unwrap();
        // Ansible's hash filter defaults to sha1
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_checksum() {
        let result = checksum(Value::from("hello"));
        // checksum uses SHA-1
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_password_hash_format() {
        let result =
            password_hash("secret".to_string(), Some("sha512".to_string()), None, None).unwrap();
        // Should start with $6$ for SHA-512
        assert!(result.starts_with("$6$"));
    }

    #[test]
    fn test_password_hash_with_salt() {
        let result = password_hash(
            "secret".to_string(),
            Some("sha256".to_string()),
            Some("testsalt".to_string()),
            None,
        )
        .unwrap();
        // Should start with $5$ for SHA-256 and contain the salt
        assert!(result.starts_with("$5$testsalt$"));
    }

    /// Reference vectors from the SHA-crypt specification; identical output to
    /// glibc crypt(3) and `openssl passwd -5/-6`.
    #[test]
    fn test_password_hash_matches_crypt_vectors() {
        assert_eq!(
            password_hash(
                "Hello world!".to_string(),
                Some("sha512".to_string()),
                Some("saltstring".to_string()),
                None,
            )
            .unwrap(),
            "$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1"
        );
        assert_eq!(
            password_hash(
                "Hello world!".to_string(),
                Some("sha256".to_string()),
                Some("saltstring".to_string()),
                None,
            )
            .unwrap(),
            "$5$saltstring$5B8vYYiY.CVt1RlTTf8KbXBH3hsxY/GNooZaBBGWEc5"
        );
    }

    #[test]
    fn test_password_hash_with_rounds() {
        assert_eq!(
            password_hash(
                "Hello world!".to_string(),
                Some("sha512".to_string()),
                Some("saltstringsaltst".to_string()),
                Some(10_000),
            )
            .unwrap(),
            "$6$rounds=10000$saltstringsaltst$OW1/O6BYHV6BcXZu8QVeXbDWra3Oeqh0sbHbbMCVNSnCM/UrjmM0Dp8vOuZeHBy/YTBmSK6H9qs/y3RnOaw5v."
        );
        assert_eq!(
            password_hash(
                "Hello world!".to_string(),
                Some("sha256".to_string()),
                Some("saltstringsaltst".to_string()),
                Some(10_000),
            )
            .unwrap(),
            "$5$rounds=10000$saltstringsaltst$3xv.VbSHBb41AL9AvLeujZkZRBAwqFMz2.opqey6IcA"
        );
    }

    #[test]
    fn test_password_hash_rejects_unsupported_scheme() {
        let err = password_hash("secret".to_string(), Some("bcrypt".to_string()), None, None)
            .unwrap_err();
        assert!(err.to_string().contains("unsupported hash type"));
    }

    #[test]
    fn test_password_hash_rejects_invalid_salt() {
        let err = password_hash(
            "secret".to_string(),
            Some("sha512".to_string()),
            Some("bad$salt".to_string()),
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid salt character"));
    }

    #[test]
    fn test_password_hash_truncates_long_salt() {
        let result = password_hash(
            "secret".to_string(),
            Some("sha512".to_string()),
            Some("0123456789abcdefTOOLONG".to_string()),
            None,
        )
        .unwrap();
        assert!(result.starts_with("$6$0123456789abcdef$"));
    }

    #[test]
    fn test_hash_rejects_unknown_algorithm() {
        let err = hash(Value::from("hello"), Some("blake2b".to_string())).unwrap_err();
        assert!(err.to_string().contains("unsupported algorithm"));
    }

    #[test]
    fn test_empty_string_hash() {
        let result = md5_filter(Value::from(""));
        // Known MD5 of empty string
        assert_eq!(result, "d41d8cd98f00b204e9800998ecf8427e");
    }
}

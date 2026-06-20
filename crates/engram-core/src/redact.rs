//! Secret redaction.
//!
//! Engram captures real session transcripts, which routinely contain API keys,
//! tokens, and `.env` contents. Every chunk passes through [`scrub`] before it
//! is written to SQLite or markdown, so secrets never land in storage in the
//! clear. We err toward over-redaction: a false positive loses a little context,
//! a false negative leaks a credential.
//!
//! This is pattern-based (known prefixes + `KEY=value` assignments), not a
//! guarantee. It catches the common, high-impact cases; see issue #1.

use std::sync::LazyLock;

use regex::Regex;

/// (compiled pattern, replacement) pairs, applied in order.
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let p = |re: &str| Regex::new(re).expect("valid redaction regex");
    vec![
        // PEM private key blocks (multi-line).
        (
            p(r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----"),
            "[REDACTED:private-key]",
        ),
        // JSON Web Tokens.
        (
            p(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"),
            "[REDACTED:jwt]",
        ),
        // Anthropic keys (must precede the generic sk- rule).
        (p(r"\bsk-ant-[A-Za-z0-9_-]{20,}"), "[REDACTED:api-key]"),
        // OpenAI-style keys.
        (p(r"\bsk-[A-Za-z0-9]{20,}"), "[REDACTED:api-key]"),
        // GitHub tokens (PAT, OAuth, app, refresh) and fine-grained PATs.
        (
            p(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}"),
            "[REDACTED:github-token]",
        ),
        (p(r"\bgithub_pat_[A-Za-z0-9_]{20,}"), "[REDACTED:github-token]"),
        // Slack tokens.
        (p(r"\bxox[baprs]-[A-Za-z0-9-]{10,}"), "[REDACTED:slack-token]"),
        // Google API keys.
        (p(r"\bAIza[0-9A-Za-z_-]{35}"), "[REDACTED:google-key]"),
        // AWS access key IDs.
        (p(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"), "[REDACTED:aws-key]"),
        // HTTP bearer tokens.
        (
            p(r"(?i)\bbearer\s+[A-Za-z0-9._-]{12,}"),
            "Bearer [REDACTED:token]",
        ),
        // KEY=value / KEY: value assignments where the name looks secret.
        (
            p(r#"(?i)\b([A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|PWD|CREDENTIAL|PRIVATE)[A-Z0-9_]*)\s*[:=]\s*["']?([^\s"']{6,})["']?"#),
            "$1=[REDACTED:secret]",
        ),
    ]
});

/// Replace recognizable secrets in `text` with labelled placeholders.
pub fn scrub(text: &str) -> String {
    let mut out = text.to_string();
    for (re, replacement) in PATTERNS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_provider_keys() {
        let s = scrub("token is sk-ant-api03-abcdefghijklmnopqrstuvwxyz123456 ok");
        assert!(s.contains("[REDACTED:api-key]"));
        assert!(!s.contains("sk-ant-api03"));
    }

    #[test]
    fn redacts_github_and_aws() {
        let s = scrub("ghp_0123456789abcdefghijklmnopqrstuvwxyz and AKIAIOSFODNN7EXAMPLE");
        assert!(s.contains("[REDACTED:github-token]"));
        assert!(s.contains("[REDACTED:aws-key]"));
    }

    #[test]
    fn redacts_env_assignments_keeps_name() {
        let s = scrub("DATABASE_PASSWORD=hunter2supersecret");
        assert!(s.contains("DATABASE_PASSWORD=[REDACTED:secret]"));
        assert!(!s.contains("hunter2supersecret"));
    }

    #[test]
    fn redacts_private_key_block() {
        let s = scrub("-----BEGIN RSA PRIVATE KEY-----\nMIIabc\n-----END RSA PRIVATE KEY-----");
        assert_eq!(s, "[REDACTED:private-key]");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        let text = "We decided to use GKE because Minikube was too slow for the operator loop.";
        assert_eq!(scrub(text), text);
    }
}

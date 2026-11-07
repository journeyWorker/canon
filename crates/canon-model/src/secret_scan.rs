//! Where a text holds secret material: a pure, allocation-light span scanner.
//!
//! [`scan`] reads a text once per rule and returns the byte ranges that look like a secret, in start order. The six rules are hand-written
//! (no regular expression, no dependency): each is a prefix, a character class, a minimum length and a left boundary, so a word that only
//! contains `risk-`, `task-` or `akia` is not a secret. Matching is case-sensitive; only the key names of [`SecretKind::NamedValue`] ignore case.
//!
//! The scanner finds and locates; it does not redact. The position model (`start`, `end`, `kind`) is the one a redactor would take, but no
//! caller redacts, so there is no function that does.
//!
//! | rule | [`SecretKind`] | shape |
//! |---|---|---|
//! | `aws-access-key-id` | [`SecretKind::AwsAccessKeyId`] | left boundary, `AKIA`, 16 of `A-Z0-9`, right boundary |
//! | `github-token` | [`SecretKind::GithubToken`] | left boundary, `ghp_`, 36 or more of `A-Za-z0-9` |
//! | `github-fine-grained-token` | [`SecretKind::GithubFineGrainedToken`] | left boundary, `github_pat_`, 22 or more of `A-Za-z0-9_` |
//! | `api-secret-key` | [`SecretKind::ApiSecretKey`] | left boundary, `sk-`, 20 or more of `A-Za-z0-9_-` with at least one digit |
//! | `private-key` | [`SecretKind::PrivateKey`] | a `-----BEGIN …PRIVATE KEY…-----` header line, through `-----END …-----` when there is one |
//! | `named-value` | [`SecretKind::NamedValue`] | `api_key`, `apikey`, `access_token` or `secret_key`, an `=` or a quoted `:`, a value of 8 or more of `A-Za-z0-9._~+/=-` that is not all upper case |
//!
//! The left boundary is "no byte before, or a byte that is not an ASCII letter, digit or `_`".
//!
//! Every rule reads at most [`CANDIDATE_WINDOW`] bytes ([`HEADER_WINDOW`] for a PEM header line) from the byte a candidate starts at, and a
//! rule that failed on a long run remembers what it learned about the run, so the scan is linear in the length of the text.

/// The most bytes a rule reads from the start of a candidate.
pub const CANDIDATE_WINDOW: usize = 512;
/// The most bytes a PEM header line (`-----BEGIN …-----`) or trailer line may take.
pub const HEADER_WINDOW: usize = 80;

const AWS_PREFIX: &[u8] = b"AKIA";
const AWS_BODY_LEN: usize = 16;
const GITHUB_PREFIX: &[u8] = b"ghp_";
const GITHUB_MIN_LEN: usize = 36;
const GITHUB_FINE_PREFIX: &[u8] = b"github_pat_";
const GITHUB_FINE_MIN_LEN: usize = 22;
const API_KEY_PREFIX: &[u8] = b"sk-";
const API_KEY_MIN_LEN: usize = 20;
const PEM_BEGIN: &[u8] = b"-----BEGIN ";
const PEM_END: &[u8] = b"-----END ";
const PEM_DASHES: &[u8] = b"-----";
const PEM_PRIVATE_KEY: &[u8] = b"PRIVATE KEY";
const KEY_NAMES: [&[u8]; 4] = [b"api_key", b"apikey", b"access_token", b"secret_key"];
const NAMED_VALUE_MIN_LEN: usize = 8;

/// The rule that matched a [`SecretSpan`], in the order the rules rank when two spans start at the same byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// An AWS access key id (`AKIA` and 16 upper-case letters or digits).
    AwsAccessKeyId,
    /// A GitHub personal access token (`ghp_`).
    GithubToken,
    /// A GitHub fine-grained token (`github_pat_`).
    GithubFineGrainedToken,
    /// An API secret key (`sk-`, including the `sk-proj-` form).
    ApiSecretKey,
    /// A private key: the header line through the end line, or the header line alone.
    PrivateKey,
    /// The value after a key name such as `api_key=` or `"access_token": `.
    NamedValue,
}

impl SecretKind {
    /// The rule's name, as a refusal prints it.
    #[must_use]
    pub const fn rule_name(self) -> &'static str {
        match self {
            Self::AwsAccessKeyId => "aws-access-key-id",
            Self::GithubToken => "github-token",
            Self::GithubFineGrainedToken => "github-fine-grained-token",
            Self::ApiSecretKey => "api-secret-key",
            Self::PrivateKey => "private-key",
            Self::NamedValue => "named-value",
        }
    }
}

/// A byte range of the scanned text that holds secret material; `start..end` always lies on character boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSpan {
    /// The first byte of the secret.
    pub start: usize,
    /// One past the last byte of the secret.
    pub end: usize,
    /// The rule that matched; of two overlapping spans, the one that starts first.
    pub kind: SecretKind,
}

/// The secret spans of `text`, in start order. Spans that overlap are one span of the earlier kind; spans that only touch stay two.
#[must_use]
pub fn scan(text: &str) -> Vec<SecretSpan> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    scan_aws_access_key(bytes, &mut found);
    scan_github_token(bytes, &mut found);
    scan_github_fine_grained_token(bytes, &mut found);
    scan_api_secret_key(bytes, &mut found);
    scan_private_key(bytes, &mut found);
    scan_named_value(bytes, &mut found);
    merge(found)
}

/// The 1-based line of the byte `offset` of `text` (a [`SecretSpan::start`]).
#[must_use]
pub fn line_of(text: &str, offset: usize) -> usize {
    1 + text.as_bytes()[..offset.min(text.len())].iter().filter(|byte| **byte == b'\n').count()
}

/// Sorts by start (the rule order breaks a tie, because the rules push in table order and the sort is stable) and joins overlaps.
fn merge(mut found: Vec<SecretSpan>) -> Vec<SecretSpan> {
    found.sort_by_key(|span| span.start);
    let mut merged: Vec<SecretSpan> = Vec::with_capacity(found.len());
    for span in found {
        match merged.last_mut() {
            Some(last) if span.start < last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn left_boundary(bytes: &[u8], at: usize) -> bool {
    at == 0 || !is_word(bytes[at - 1])
}

/// The first occurrence of `needle` at or after `from`.
fn find_from(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let first = needle[0];
    let mut at = from;
    while at < bytes.len() {
        at += bytes[at..].iter().position(|byte| *byte == first)?;
        if bytes[at..].starts_with(needle) {
            return Some(at);
        }
        at += 1;
    }
    None
}

/// How many bytes from `from` (and before `limit`) are in the class.
fn run_len(bytes: &[u8], from: usize, limit: usize, class: fn(u8) -> bool) -> usize {
    let limit = limit.min(bytes.len());
    if from >= limit {
        return 0;
    }
    bytes[from..limit].iter().take_while(|byte| class(**byte)).count()
}

fn is_upper_or_digit(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit()
}

fn scan_aws_access_key(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    let end_of_body = AWS_PREFIX.len() + AWS_BODY_LEN;
    let mut from = 0;
    while let Some(at) = find_from(bytes, from, AWS_PREFIX) {
        let end = at + end_of_body;
        let body_ok = end <= bytes.len() && bytes[at + AWS_PREFIX.len()..end].iter().all(|byte| is_upper_or_digit(*byte));
        if left_boundary(bytes, at) && body_ok && bytes.get(end).is_none_or(|next| !is_word(*next)) {
            out.push(SecretSpan { start: at, end, kind: SecretKind::AwsAccessKeyId });
            from = end;
        } else {
            from = at + 1;
        }
    }
}

/// The shared shape of the two GitHub rules: a prefix and a run of at least `min_len` bytes of a class.
fn scan_prefixed_run(bytes: &[u8], prefix: &[u8], min_len: usize, class: fn(u8) -> bool, kind: SecretKind, out: &mut Vec<SecretSpan>) {
    let mut from = 0;
    while let Some(at) = find_from(bytes, from, prefix) {
        if left_boundary(bytes, at) {
            let body = at + prefix.len();
            let len = run_len(bytes, body, at + CANDIDATE_WINDOW, class);
            if len >= min_len {
                out.push(SecretSpan { start: at, end: body + len, kind });
                from = body + len;
                continue;
            }
        }
        from = at + 1;
    }
}

fn scan_github_token(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    scan_prefixed_run(bytes, GITHUB_PREFIX, GITHUB_MIN_LEN, |byte| byte.is_ascii_alphanumeric(), SecretKind::GithubToken, out);
}

fn scan_github_fine_grained_token(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    scan_prefixed_run(bytes, GITHUB_FINE_PREFIX, GITHUB_FINE_MIN_LEN, is_word, SecretKind::GithubFineGrainedToken, out);
}

fn is_key_char(byte: u8) -> bool {
    is_word(byte) || byte == b'-'
}

/// What the `sk-` rule has learned about the class run it is reading, so a text of many `sk-` that never reaches a digit is not read
/// `CANDIDATE_WINDOW` bytes per candidate: where the run ends, and where its first digit at or after a position is.
struct KeyRun {
    /// A class run known to cover `run_from..run_end`.
    run_from: usize,
    run_end: usize,
    /// The first digit at or after `digit_from` (`run_end` of that run when it has none).
    digit_from: usize,
    digit_at: usize,
}

impl KeyRun {
    fn new() -> Self {
        Self { run_from: 0, run_end: 0, digit_from: 0, digit_at: 0 }
    }

    /// The end of the class run that holds `body` (the byte after the prefix), and the first digit of the run at or after `body`.
    fn locate(&mut self, bytes: &[u8], body: usize) -> (usize, usize) {
        if !(self.run_from <= body && body < self.run_end) {
            self.run_from = body;
            self.run_end = body + run_len(bytes, body, bytes.len(), is_key_char);
            self.digit_from = body;
            self.digit_at = self.next_digit(bytes, body);
        } else if !(self.digit_from <= body && body <= self.digit_at) {
            self.digit_from = body;
            self.digit_at = self.next_digit(bytes, body);
        }
        (self.run_end, self.digit_at)
    }

    fn next_digit(&self, bytes: &[u8], from: usize) -> usize {
        bytes[from.min(self.run_end)..self.run_end].iter().position(u8::is_ascii_digit).map_or(self.run_end, |offset| from + offset)
    }
}

fn scan_api_secret_key(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    let mut run = KeyRun::new();
    let mut from = 0;
    while let Some(at) = find_from(bytes, from, API_KEY_PREFIX) {
        if left_boundary(bytes, at) {
            let body = at + API_KEY_PREFIX.len();
            let (run_end, digit_at) = run.locate(bytes, body);
            let end = run_end.min(at + CANDIDATE_WINDOW);
            if end.saturating_sub(body) >= API_KEY_MIN_LEN && digit_at < end {
                out.push(SecretSpan { start: at, end, kind: SecretKind::ApiSecretKey });
                from = end;
                continue;
            }
        }
        from = at + 1;
    }
}

/// The end of a dashed line (`marker`, upper-case words and spaces, `-----`) that starts at `at` and fits in [`HEADER_WINDOW`], with the
/// range of its words.
fn dashed_line(bytes: &[u8], at: usize, marker: &[u8]) -> Option<(usize, std::ops::Range<usize>)> {
    let words_from = at + marker.len();
    let limit = (at + HEADER_WINDOW).min(bytes.len());
    let words_len = run_len(bytes, words_from, limit, |byte| byte.is_ascii_uppercase() || byte == b' ');
    let words_to = words_from + words_len;
    let end = words_to + PEM_DASHES.len();
    (end <= limit && bytes[words_to..end] == *PEM_DASHES).then_some((end, words_from..words_to))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

fn scan_private_key(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    // The first `-----END ` at or after a position, or none: remembered as (searched from, found), valid for any later start up to `found`.
    let mut end_marker: Option<(usize, Option<usize>)> = None;
    let mut from = 0;
    while let Some(at) = find_from(bytes, from, PEM_BEGIN) {
        let Some((header_end, _)) = dashed_line(bytes, at, PEM_BEGIN).filter(|(_, words)| contains(&bytes[words.clone()], PEM_PRIVATE_KEY)) else {
            from = at + 1;
            continue;
        };
        let marker = match end_marker {
            Some((searched_from, found)) if searched_from <= header_end && found.is_none_or(|marker| header_end <= marker) => found,
            _ => {
                let found = find_from(bytes, header_end, PEM_END);
                end_marker = Some((header_end, found));
                found
            }
        };
        let end = marker.and_then(|marker| dashed_line(bytes, marker, PEM_END)).map_or(header_end, |(end, _)| end);
        out.push(SecretSpan { start: at, end, kind: SecretKind::PrivateKey });
        from = end;
    }
}

fn is_blank(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

fn is_quote(byte: u8) -> bool {
    byte == b'"' || byte == b'\''
}

fn is_named_value_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

/// The value after the key name that ends at `name_end`: `[quote] [blanks] (= | :) [blanks] [quote] value`, read within `limit`.
fn named_value(bytes: &[u8], name_end: usize, limit: usize) -> Option<std::ops::Range<usize>> {
    let limit = limit.min(bytes.len());
    let skip_blanks = |from: usize| from + run_len(bytes, from, limit, is_blank);
    let mut at = name_end;
    if at < limit && is_quote(bytes[at]) {
        at += 1;
    }
    at = skip_blanks(at);
    let separator = *bytes[..limit].get(at)?;
    if separator != b'=' && separator != b':' {
        return None;
    }
    at = skip_blanks(at + 1);
    let quoted = at < limit && is_quote(bytes[at]);
    if quoted {
        at += 1;
    }
    if separator == b':' && !quoted {
        return None;
    }
    let len = run_len(bytes, at, limit, is_named_value_char);
    let value = at..at + len;
    // A value of upper-case letters and `_` is a placeholder (`YOUR_API_KEY`), not a key.
    let placeholder = bytes[value.clone()].iter().all(|byte| byte.is_ascii_uppercase() || *byte == b'_');
    (len >= NAMED_VALUE_MIN_LEN && !placeholder).then_some(value)
}

fn scan_named_value(bytes: &[u8], out: &mut Vec<SecretSpan>) {
    let mut at = 0;
    while at < bytes.len() {
        let starts_name = matches!(bytes[at], b'a' | b'A' | b's' | b'S') && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric());
        let name = starts_name.then(|| KEY_NAMES.iter().find(|name| bytes.get(at..at + name.len()).is_some_and(|text| text.eq_ignore_ascii_case(name)))).flatten();
        if let Some(value) = name.and_then(|name| named_value(bytes, at + name.len(), at + CANDIDATE_WINDOW)) {
            at = value.end;
            out.push(SecretSpan { start: value.start, end: value.end, kind: SecretKind::NamedValue });
        } else {
            at += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(usize, usize, SecretKind)> {
        scan(text).into_iter().map(|span| (span.start, span.end, span.kind)).collect()
    }

    fn rejected(text: &str) -> bool {
        !scan(text).is_empty()
    }

    fn upper(len: usize) -> String {
        "A1".repeat(len).chars().take(len).collect()
    }

    #[test]
    fn aws_access_key_id_needs_sixteen_characters_and_both_boundaries() {
        let key = format!("AKIA{}", upper(16));
        assert_eq!(kinds(&format!("id: {key} end")), [(4, 24, SecretKind::AwsAccessKeyId)]);
        assert!(!rejected(&format!("AKIA{}", upper(15))), "fifteen is below the minimum");
        assert!(rejected(&format!("AKIA{}", upper(16))));
        assert!(!rejected(&format!("AKIA{}", upper(17))), "the right boundary fails");
        assert!(!rejected(&format!("x{key}")), "the left boundary fails");
        assert!(!rejected(&format!("_{key}")), "an underscore is a word byte");
        assert!(rejected(&format!("-{key}")), "a hyphen is a boundary");
        assert!(!rejected(&format!("AKIA{}", upper(16).to_lowercase())), "lower case is not a key id");
        assert!(!rejected("Akiashi AKIATSU akiaakiaakiaakiaakia"));
    }

    #[test]
    fn github_token_needs_thirty_six_characters() {
        let token = |len: usize| format!("ghp_{}", "a1".repeat(len).chars().take(len).collect::<String>());
        assert_eq!(kinds(&format!("t={}.", token(36))), [(2, 42, SecretKind::GithubToken)]);
        assert!(!rejected(&token(35)));
        assert!(rejected(&token(36)));
        assert!(rejected(&token(100)));
        assert!(!rejected(&format!("x{}", token(36))), "the left boundary fails");
        assert!(!rejected("ghp_ ghp_"), "a bare prefix is a word");
        assert!(!rejected(&format!("ghp_{}-", "a".repeat(35))), "a hyphen ends the run");
    }

    #[test]
    fn github_fine_grained_token_takes_underscores_from_twenty_two_characters() {
        let token = |len: usize| format!("github_pat_{}", "a_1".repeat(len).chars().take(len).collect::<String>());
        assert_eq!(kinds(&format!("({})", token(22))), [(1, 34, SecretKind::GithubFineGrainedToken)]);
        assert!(!rejected(&token(21)));
        assert!(rejected(&token(22)));
        assert!(!rejected(&format!("a{}", token(22))), "the left boundary fails");
        assert!(!rejected("github_pat_"));
    }

    #[test]
    fn api_secret_key_needs_twenty_characters_with_a_digit() {
        let key = |len: usize| format!("sk-{}", "ab1-".repeat(len).chars().take(len).collect::<String>());
        assert_eq!(kinds(&format!("key {} ", key(20))), [(4, 27, SecretKind::ApiSecretKey)]);
        assert!(!rejected(&key(19)));
        assert!(rejected(&key(20)));
        assert!(rejected("sk-proj-abcdEFGH_ijkl-mnop1234"), "the sk-proj- form has hyphens and underscores");
        assert!(!rejected("sk-live-123456"));
        assert!(!rejected("sk-documentation-for-onboarding-flow"), "a long run of words has no digit");
        assert!(!rejected(&format!("sk-{}", "a".repeat(40))));
        assert!(rejected(&format!("sk-{}1", "a".repeat(30))), "a digit anywhere in the run counts");
        assert!(!rejected(&format!("x{}", key(20))), "the left boundary fails");
        for word in ["risk-high", "RISK-HIGH", "task-list", "disk-usage", "mask-all", "ask-user"] {
            assert!(!rejected(&format!("{word}-0123456789-0123456789-0123456789")), "{word}");
        }
    }

    #[test]
    fn private_key_reads_the_header_and_runs_to_the_end_line() {
        let pem = "intro\n-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----\nafter";
        let start = pem.find("-----BEGIN").unwrap();
        assert_eq!(kinds(pem), [(start, pem.find("\nafter").unwrap(), SecretKind::PrivateKey)]);
        let header = "-----BEGIN OPENSSH PRIVATE KEY-----";
        assert_eq!(kinds(&format!("{header} body without an end")), [(0, header.len(), SecretKind::PrivateKey)], "no end line: the header alone");
        for header in ["-----BEGIN PRIVATE KEY-----", "-----BEGIN ENCRYPTED PRIVATE KEY-----", "-----BEGIN PGP PRIVATE KEY BLOCK-----", "-----BEGIN DSA PRIVATE KEY-----"] {
            assert!(rejected(header), "{header}");
        }
        for header in ["-----BEGIN CERTIFICATE-----", "-----BEGIN PUBLIC KEY-----", "-----BEGIN private key-----", "-----BEGIN PRIVATE KEY----", "-----BEGIN PRIVATE KEY"] {
            assert!(!rejected(header), "{header}");
        }
        let padded = |len: usize| format!("-----BEGIN {} PRIVATE KEY-----", "A".repeat(len));
        let fits = 80 - padded(0).len();
        assert_eq!(padded(fits).len(), HEADER_WINDOW);
        assert!(rejected(&padded(fits)), "a header of 80 bytes is read");
        assert!(!rejected(&padded(fits + 1)), "a header of 81 bytes is not");
    }

    #[test]
    fn private_key_with_an_unreadable_end_line_is_the_header_alone() {
        let text = "-----BEGIN PRIVATE KEY-----\n-----END lower-----";
        assert_eq!(kinds(text), [(0, 27, SecretKind::PrivateKey)]);
    }

    #[test]
    fn named_value_reads_the_value_after_a_key_name() {
        assert_eq!(kinds("api_key=abcd1234"), [(8, 16, SecretKind::NamedValue)]);
        assert_eq!(kinds("API_KEY = \"abcd1234\""), [(11, 19, SecretKind::NamedValue)], "case of the name, blanks around =, a quote");
        assert_eq!(kinds("export OPENAI_API_KEY=abcd1234"), [(22, 30, SecretKind::NamedValue)], "an environment variable prefix");
        assert_eq!(kinds(r#"{"access_token": "abcd1234"}"#), [(18, 26, SecretKind::NamedValue)], "a JSON pair");
        assert_eq!(kinds("secret_key:'abcd1234'"), [(12, 20, SecretKind::NamedValue)], "a colon with a quoted value");
        assert_eq!(kinds("apikey\t=\tabcd.1234"), [(9, 18, SecretKind::NamedValue)]);
        assert!(rejected("api_key=abcdefg1"), "eight characters");
        assert!(!rejected("api_key=abcdefg"), "seven characters");
        assert!(!rejected("api_key: abcd1234"), "a colon needs a quoted value");
        assert!(!rejected("xapi_key=abcd1234"), "a letter before the name");
        assert!(!rejected("1api_key=abcd1234"), "a digit before the name");
        assert!(!rejected("max_api_key_len = 4"));
        assert!(!rejected("api_key_name=abcd1234"));
    }

    #[test]
    fn named_value_ignores_placeholders_and_empty_values() {
        for line in [
            "api_key=",
            "api_key=<your key>",
            "api_key=${API_KEY}",
            "api_key=YOUR_API_KEY",
            "api_key=PLACEHOLDER",
            "API_KEY: \"YOURKEYHERE\"",
            "api_key: string",
            "api_key: \"string\"",
            "access_token = $TOKEN_VALUE",
            "secret_key=%s",
        ] {
            assert!(!rejected(line), "{line}");
        }
        assert!(rejected("api_key=PLACEHOLDER1"), "a digit makes it a value");
        assert!(rejected("api_key=Placeholder"), "mixed case makes it a value");
    }

    #[test]
    fn a_value_takes_at_most_one_window_from_the_name() {
        let text = format!("api_key={}", "a".repeat(2000));
        let spans = scan(&text);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].end - spans[0].start, CANDIDATE_WINDOW - "api_key=".len());
    }

    #[test]
    fn overlapping_spans_merge_into_the_earlier_kind_and_touching_spans_stay_apart() {
        let key = format!("sk-{}", "ab1".repeat(10));
        // `api_key="sk-…"`: the key's value and the `sk-` run are the same bytes; the rule that comes first in the table names them.
        let text = format!("api_key=\"{key}\"");
        assert_eq!(kinds(&text), [(9, 9 + key.len(), SecretKind::ApiSecretKey)]);
        let pem = "-----BEGIN PRIVATE KEY-----\napi_key=abcd1234\n-----END PRIVATE KEY-----";
        assert_eq!(kinds(pem), [(0, pem.len(), SecretKind::PrivateKey)], "a value inside a private key joins it");
        let aws = format!("AKIA{}", upper(16));
        assert_eq!(kinds(&format!("{aws} {aws}")).len(), 2, "a gap keeps two spans");
        let adjacent = format!("{aws}{aws}");
        assert_eq!(kinds(&format!("{aws},{aws}")).len(), 2);
        assert!(!rejected(&adjacent), "two key ids with no gap are not a key id");
        let touching = |start: usize, end: usize| SecretSpan { start, end, kind: SecretKind::NamedValue };
        assert_eq!(merge(vec![touching(4, 8), touching(0, 4)]).len(), 2, "spans that only touch stay two");
        assert_eq!(merge(vec![touching(4, 8), touching(0, 5)]), [touching(0, 8)], "spans that overlap are one");
    }

    #[test]
    fn spans_come_in_start_order_whatever_the_rule_order() {
        let key = format!("sk-{}", "ab1".repeat(10));
        let aws = format!("AKIA{}", upper(16));
        let text = format!("{key} {aws} -----BEGIN PRIVATE KEY----- api_key=abcd1234");
        let spans = scan(&text);
        assert_eq!(spans.iter().map(|span| span.kind).collect::<Vec<_>>(), [SecretKind::ApiSecretKey, SecretKind::AwsAccessKeyId, SecretKind::PrivateKey, SecretKind::NamedValue]);
        assert!(spans.windows(2).all(|pair| pair[0].start < pair[1].start));
    }

    #[test]
    fn spans_lie_on_character_boundaries_in_text_with_multibyte_characters() {
        let text = "비밀 🙂 api_key=\"abcd1234\" 끝 ghp_é";
        for span in scan(text) {
            assert!(text.is_char_boundary(span.start) && text.is_char_boundary(span.end), "{span:?}");
        }
        assert_eq!(scan(text).len(), 1);
    }

    #[test]
    fn line_of_counts_from_one() {
        let text = "a\nb\r\nc api_key=abcd1234";
        let span = scan(text)[0];
        assert_eq!(line_of(text, span.start), 3);
        assert_eq!(line_of(text, 0), 1);
        assert_eq!(line_of("", 5), 1);
    }

    #[test]
    fn plain_prose_holds_no_secret() {
        for line in [
            "risk-high", "RISK-HIGH", "task-list", "disk-usage", "mask-all", "ask-user", "Akiashi", "AKIATSU", "sk-live-123456",
            "sk-documentation-for-onboarding-flow", "ghp_", "github_pat_", "-----BEGIN CERTIFICATE-----", "-----BEGIN PUBLIC KEY-----", "api_key=",
            "api_key=<your key>", "api_key=${API_KEY}", "api_key=YOUR_API_KEY", "api_key: string", "max_api_key_len = 4",
        ] {
            assert!(scan(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn a_long_input_of_repeated_candidates_is_scanned_in_linear_time() {
        const MIB4: usize = 4 << 20;
        for unit in ["sk-", "AKIA", "-----BEGIN ", "sk-a-", "-----BEGIN PRIVATE KEY-----\n"] {
            let text = unit.repeat(MIB4 / unit.len());
            let spans = scan(&text);
            assert!(spans.iter().all(|span| span.end <= text.len()), "{unit:?}");
        }
    }
}

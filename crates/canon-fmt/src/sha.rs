//! Abbreviated-sha detection (S11 design D5): `canon fmt --check`
//! flags `app_sha`/`harness_sha` values shorter than the full 40-hex
//! sha grammar (`FmtFailureClass::AbbreviatedSha`) — resolving an
//! abbreviated sha against a real git history is explicitly out of
//! scope for this crate (that was `canon migrate`'s job; the tool is
//! removed per operator directive 2026-07-10, see the S11 change's
//! `design.md`).

/// Whether `s` is already a full 40-lowercase-hex sha (matches
/// [`canon_model::ids::Sha`]'s own grammar, duplicated here in terms of
/// plain `&str` since family records keep `app_sha`/`harness_sha` as
/// loose strings, not the strict newtype — see `family::ledger`'s
/// module doc for why).
pub fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_full_sha_requires_exactly_40_lowercase_hex() {
        assert!(is_full_sha("2745ca4c889d49f11aa96c51b2f2cf01a4be0009"));
        assert!(!is_full_sha("cfa43a50"));
        assert!(!is_full_sha("2745CA4C889D49F11AA96C51B2F2CF01A4BE0009"));
    }
}

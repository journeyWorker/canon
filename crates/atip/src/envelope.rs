//! A document as one line.
//!
//! The envelope's members (`ok`, `data` or `error`, `meta`) belong to the tool
//! that builds it; what is shared is how it leaves the process: compact JSON,
//! one line, on stdout.

use serde::Serialize;

/// `document` as compact JSON on one line, without the trailing newline.
///
/// # Panics
/// When `document` does not serialize: a typed document always does, so a
/// failure is a bug in the caller's types.
pub fn line<T: Serialize + ?Sized>(document: &T) -> String {
    serde_json::to_string(document).expect("a document serializes")
}

/// Prints `line` on stdout, ended by one newline.
pub fn print_line(line: &str) {
    println!("{line}");
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::line;

    #[derive(Serialize)]
    struct Document {
        ok: bool,
        data: Vec<u8>,
    }

    #[test]
    fn a_document_is_one_compact_line_in_member_order() {
        assert_eq!(line(&Document { ok: true, data: vec![1, 2] }), r#"{"ok":true,"data":[1,2]}"#);
    }
}

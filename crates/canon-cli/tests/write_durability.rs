//! 0.39 `canon-0-39-write-durability`: the source check that no write copy is left outside `canon-store`.

use std::path::Path;

/// tier.durability.06 (F4): the fixed temp names of the four private write copies (`canon.yaml`, the legacy migration, the brief migration, the strategy activation file) are
/// gone from every crate's source; the one write path is `canon_store::write_atomic`.
#[test]
fn tier_durability_06_no_source_outside_canon_store_keeps_a_private_temp_and_rename_write() {
    fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap().flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            sources(&src, &mut found);
        }
    }
    assert!(!found.is_empty(), "the source scan found no files");
    let gone = [".canon-migrate.tmp", "canon-migrate.tmp", "\"canon-tmp\"", "md.activation"];
    let left: Vec<String> = found
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            gone.iter().find(|name| text.contains(*name)).map(|name| format!("{} still names `{name}`", path.display()))
        })
        .collect();
    assert_eq!(left, Vec::<String>::new());
}

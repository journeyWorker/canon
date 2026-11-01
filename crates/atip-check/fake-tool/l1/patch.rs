//! The generic seeded violation of a printed document: a list of JSON-patch operations applied to a document after it
//! was built from the model, so the behaviour still follows the unpatched model.
//!
//! The operations are those of RFC 6902 (`add`, `remove`, `replace`, `move`, `copy`), plus `set` (an `add` that
//! replaces) and `remove-value` (removes the elements of an array equal to `value`). A pointer segment of the form
//! `key=<text>` selects the element of an array whose member `key` (`name`, `code`, `reason`, ...) is that text, so a
//! patch need not know a position.

use serde_json::Value;

/// Applies `ops` in order to `document`.
///
/// # Errors
/// The first operation that names a place the document does not have, or is not one of the operations above.
pub fn apply(document: &mut Value, ops: &[Value]) -> Result<(), String> {
    for op in ops {
        apply_one(document, op).map_err(|reason| format!("patch {op}: {reason}"))?;
    }
    Ok(())
}

fn segments(pointer: &str) -> Vec<String> {
    if pointer.is_empty() {
        return Vec::new();
    }
    pointer
        .split('/')
        .skip(1)
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect()
}

/// The position a segment names in an array: an index, or `key=<text>`.
fn position(items: &[Value], segment: &str) -> Option<usize> {
    if let Some((key, wanted)) = segment.split_once('=') {
        return items
            .iter()
            .position(|item| item.get(key).and_then(Value::as_str) == Some(wanted));
    }
    segment.parse::<usize>().ok().filter(|index| *index < items.len())
}

fn walk<'a>(document: &'a mut Value, path: &[String]) -> Result<&'a mut Value, String> {
    let mut current = document;
    for segment in path {
        current = match current {
            Value::Object(members) => members
                .get_mut(segment)
                .ok_or_else(|| format!("no member {segment}"))?,
            Value::Array(items) => {
                let index = position(items, segment).ok_or_else(|| format!("no element {segment}"))?;
                &mut items[index]
            }
            _ => return Err(format!("{segment} is below a scalar")),
        };
    }
    Ok(current)
}

fn read(document: &mut Value, pointer: &str) -> Result<Value, String> {
    walk(document, &segments(pointer)).map(|value| value.clone())
}

fn remove(document: &mut Value, pointer: &str) -> Result<Value, String> {
    let path = segments(pointer);
    let (last, parent) = path.split_last().ok_or("the document itself cannot be removed")?;
    match walk(document, parent)? {
        Value::Object(members) => members.remove(last).ok_or_else(|| format!("no member {last}")),
        Value::Array(items) => {
            let index = position(items, last).ok_or_else(|| format!("no element {last}"))?;
            Ok(items.remove(index))
        }
        _ => Err("the parent is a scalar".to_string()),
    }
}

/// Puts `value` at `pointer`: inserts into an array (`-` appends), sets a member; `replace` demands that it exists.
fn put(document: &mut Value, pointer: &str, value: Value, existing: Existing) -> Result<(), String> {
    let path = segments(pointer);
    let Some((last, parent)) = path.split_last() else {
        *document = value;
        return Ok(());
    };
    match walk(document, parent)? {
        Value::Object(members) => {
            if existing == Existing::Required && !members.contains_key(last) {
                return Err(format!("no member {last} to replace"));
            }
            members.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(items) => {
            if last == "-" {
                items.push(value);
                return Ok(());
            }
            if existing == Existing::Insert {
                let index = last.parse::<usize>().ok().filter(|index| *index <= items.len());
                items.insert(index.ok_or_else(|| format!("no index {last}"))?, value);
                return Ok(());
            }
            let index = position(items, last).ok_or_else(|| format!("no element {last}"))?;
            items[index] = value;
            Ok(())
        }
        _ => Err("the parent is a scalar".to_string()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Existing {
    /// RFC 6902 `add`: an array element is inserted, a member is set.
    Insert,
    /// `set`: the place is replaced when it exists and created otherwise.
    Any,
    /// `replace`: the place exists.
    Required,
}

fn apply_one(document: &mut Value, op: &Value) -> Result<(), String> {
    let kind = op.get("op").and_then(Value::as_str).ok_or("no op")?;
    let path = op.get("path").and_then(Value::as_str).ok_or("no path")?;
    let value = || op.get("value").cloned().ok_or("no value");
    let from = || op.get("from").and_then(Value::as_str).ok_or("no from");
    match kind {
        "add" => put(document, path, value()?, Existing::Insert),
        "set" => put(document, path, value()?, Existing::Any),
        "replace" => put(document, path, value()?, Existing::Required),
        "remove" => remove(document, path).map(drop),
        "copy" => {
            let copy = read(document, from()?)?;
            put(document, path, copy, Existing::Insert)
        }
        "move" => {
            let moved = remove(document, from()?)?;
            put(document, path, moved, Existing::Insert)
        }
        "remove-value" => {
            let wanted = value()?;
            match walk(document, &segments(path))? {
                Value::Array(items) => {
                    items.retain(|item| *item != wanted);
                    Ok(())
                }
                _ => Err("not an array".to_string()),
            }
        }
        other => Err(format!("unknown op {other}")),
    }
}

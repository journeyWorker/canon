// Presentation-only DOM helper shared by all seven dashboard panels — builds an
// HTML <table> from column defs + row objects, plus the two annotation
// affordances a web surface has and the markdown report does not: a
// per-column tooltip and a short note under the table. Carries no query
// or aggregation logic of its own (design.md: panels are thin
// SELECT/filter over the snapshot; this module only turns already-
// queried rows into markup).
//
// s42 (`close-the-open-loops`) re-review: the dashboard is a SECOND
// surface over the same marts as `.canon/REPORT.md`, and a reader must
// not learn something different from it. The markdown report states
// each panel's exact computation in prose
// (`crates/canon-report/src/render.rs`); `note` is where the same
// statement lands here, and `description` is where a single column's
// rule lands. Neither is decoration: a friendly column label that
// asserts more than the mart computes is the defect class this release
// line has repeatedly shipped.
export interface ColumnDef {
  key: string;
  label: string;
  /** Optional per-cell formatter; defaults to a bigint/null-safe stringify. */
  format?: (value: unknown) => string;
  /**
   * Exact statement of what this column computes, surfaced as the
   * header cell's `title` tooltip. Plain prose, no backticks — a
   * `title` attribute renders verbatim.
   */
  description?: string;
}

// Splits a note on backtick-delimited spans so the same sentence the
// markdown report prints can be pasted here and still render its
// identifiers as code rather than as stray backticks.
function noteElement(note: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = "panel-note";
  const parts = note.split("`");
  for (const [index, part] of parts.entries()) {
    if (part === "") continue;
    if (index % 2 === 1) {
      const code = document.createElement("code");
      code.textContent = part;
      p.append(code);
    } else {
      p.append(document.createTextNode(part));
    }
  }
  return p;
}

function defaultFormat(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "boolean") return value ? "yes" : "no";
  return String(value);
}

function cellFor(column: ColumnDef, value: unknown): HTMLTableCellElement {
  const td = document.createElement("td");
  if (typeof value === "boolean") {
    td.textContent = value ? "yes" : "no";
    td.className = value ? "bool-true" : "bool-false";
  } else {
    td.textContent = (column.format ?? defaultFormat)(value);
  }
  return td;
}

export function renderTable(
  container: HTMLElement,
  columns: ColumnDef[],
  rows: Record<string, unknown>[],
  note?: string,
): void {
  container.replaceChildren();

  // The note describes the METRIC, not the data, so an empty snapshot
  // still gets it — otherwise the one reader most likely to misread a
  // column ("why is this zero?") is the one who never sees the caveat.
  if (rows.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "no rows in this snapshot";
    container.append(empty);
    if (note) container.append(noteElement(note));
    return;
  }

  const table = document.createElement("table");
  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  for (const column of columns) {
    const th = document.createElement("th");
    th.textContent = column.label;
    if (column.description) {
      th.title = column.description;
      th.className = "col-annotated";
    }
    headRow.append(th);
  }
  thead.append(headRow);

  const tbody = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    for (const column of columns) {
      tr.append(cellFor(column, row[column.key]));
    }
    tbody.append(tr);
  }

  table.append(thead, tbody);
  // The scroll wrapper is the table's alone — a panel's note must stay
  // put and wrap, not slide out of view when a wide table is scrolled.
  const scroll = document.createElement("div");
  scroll.className = "table-scroll";
  scroll.append(table);
  container.append(scroll);
  if (note) container.append(noteElement(note));
}

// Presentation-only DOM helper shared by all five panels — builds an
// HTML <table> from column defs + row objects. Carries no query or
// aggregation logic of its own (design.md: panels are thin SELECT/filter
// over the snapshot; this module only turns already-queried rows into
// markup).
export interface ColumnDef {
  key: string;
  label: string;
  /** Optional per-cell formatter; defaults to a bigint/null-safe stringify. */
  format?: (value: unknown) => string;
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
): void {
  container.replaceChildren();

  if (rows.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "no rows in this snapshot";
    container.append(empty);
    return;
  }

  const table = document.createElement("table");
  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  for (const column of columns) {
    const th = document.createElement("th");
    th.textContent = column.label;
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
  container.append(table);
}

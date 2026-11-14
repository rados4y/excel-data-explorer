import type { ColumnPreview, Field, Source } from "./types";
export function sourceColumns(source?: Source): string[] {
  return Object.entries(source?.fields || {})
    .filter(([id]) => id !== "__row_id")
    .sort(
      ([a, fa], [b, fb]) =>
        (fa.column_index ?? (Number(a.match(/^column_(\d+)/)?.[1]) || 99999)) -
        (fb.column_index ?? (Number(b.match(/^column_(\d+)/)?.[1]) || 99999)),
    )
    .map(([id]) => id);
}
const rowField = (): Field => ({
  column: "__row_id",
  column_index: null,
  label: "Row",
  type: "string",
  required: true,
  trim: false,
  searchable: false,
  filterable: false,
});
/** Preserve logical fields by unique header or explicit position; never reuse one field twice. */
export function refreshWorksheet(
  source: Source,
  columns: ColumnPreview[],
): Source {
  const fields: Record<string, Field> = {},
    used = new Set<string>(),
    reserved = new Set(Object.keys(source.fields));
  if (source.key_mode === "row_number")
    fields.__row_id = source.fields.__row_id || rowField();
  for (const col of columns) {
    const match = Object.entries(source.fields).find(
      ([id, f]) =>
        id !== "__row_id" &&
        !used.has(id) &&
        (f.column_index
          ? f.column_index === col.column_index
          : f.column === col.column),
    );
    let id = match?.[0] || col.id;
    if (!match) {
      let suffix = 1;
      while (reserved.has(id) || used.has(id)) {
        id = `${col.id}_new_${suffix++}`;
      }
    }
    used.add(id);
    const old = match?.[1];
    fields[id] = {
      column: col.column.trim() ? col.column : col.label,
      column_index:
        old?.column_index ||
        !col.column.trim() ||
        columns.filter((c) => c.column === col.column).length > 1
          ? col.column_index
          : null,
      type: old?.type || "string",
      label: old?.label && old.label !== old.column ? old.label : col.label,
      required: old?.required || false,
      trim: old?.trim || false,
      searchable: old?.searchable ?? true,
      filterable: old?.filterable ?? true,
    };
  }
  if (!fields[source.key]) {
    fields.__row_id = rowField();
    return { ...source, fields, key: "__row_id", key_mode: "row_number" };
  }
  return { ...source, fields };
}

import { expect, test } from "vitest";
import { refreshWorksheet, sourceColumns } from "./spaceModel";
import type { ColumnPreview, Field, Source } from "./types";
function field(column: string): Field {
  return {
    column,
    type: "string",
    label: column,
    required: false,
    trim: false,
    searchable: true,
    filterable: true,
  };
}
function column(name: string, index: number): ColumnPreview {
  return {
    id: `column_${index}`,
    column: name,
    label: name || `Column ${index}`,
    column_index: index,
  };
}
function source(): Source {
  return {
    id: "sheet_1",
    sheet: "Orders",
    path: "orders.xlsx",
    entity: "orders",
    namespace: "orders",
    header_row: 1,
    key: "column_1",
    key_mode: "field",
    fields: {
      column_1: { ...field("ID"), required: true },
      column_2: {
        ...field("Amount"),
        label: "Contract value",
        type: "decimal",
      },
    },
  };
}
test("refresh headers preserves logical keys and custom types/labels after reordering", () => {
  const next = refreshWorksheet(source(), [
    column("Amount", 1),
    column("ID", 2),
  ]);
  expect(next.fields.column_1.column).toBe("ID");
  expect(next.fields.column_2.type).toBe("decimal");
  expect(next.fields.column_2.label).toBe("Contract value");
  expect(next.key).toBe("column_1");
  expect(next.fields.column_1.column_index).toBeNull();
});
test("new duplicate and blank headers receive distinct field identities and physical mappings", () => {
  const next = refreshWorksheet(source(), [
    column("ID", 1),
    column("ID", 2),
    column("", 3),
  ]);
  const fields = Object.entries(next.fields).filter(
    ([id]) => id !== "__row_id",
  );
  expect(fields).toHaveLength(3);
  expect(fields.map(([, f]) => f.column_index)).toEqual([1, 2, 3]);
  expect(new Set(fields.map(([id]) => id)).size).toBe(3);
});
test("removing the selected key uses an explicit generated row fallback", () => {
  const next = refreshWorksheet(source(), [column("Customer", 1)]);
  expect(next.key_mode).toBe("row_number");
  expect(next.fields.__row_id.required).toBe(true);
  expect(next.fields.column_1).toBeUndefined();
  expect(sourceColumns(next)).toHaveLength(1);
});

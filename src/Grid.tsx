import { useMemo, useRef } from "react";
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type ColumnSizingState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowUp,
  ArrowDown,
  ChevronsUpDown,
  FileSpreadsheet,
} from "lucide-react";
import { label } from "./api";
import type { DataRecord, Field } from "./types";
interface Props {
  rows: DataRecord[];
  fields: Record<string, Field>;
  columns: string[];
  selectedId?: string | null;
  sort?: string;
  direction?: "asc" | "desc";
  onSort?: (key: string) => void;
  onSelect?: (r: DataRecord) => void;
  onOpen: (r: DataRecord) => void;
  widths?: ColumnSizingState;
  onWidths?: (s: ColumnSizingState) => void;
  loading?: boolean;
}
export function Grid(p: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const columns = useMemo(
    () =>
      p.columns.map((key) => ({
        id: key,
        accessorFn: (r: DataRecord) => r.fields[key],
        header: p.fields[key]?.label || label(key),
        size: key === "description" ? 330 : key === "customer" ? 220 : 145,
        minSize: 90,
        cell: ({ getValue }: { getValue: () => unknown }) => {
          const v = getValue();
          if (v == null) return <span className="muted">—</span>;
          if (key === "status")
            return (
              <span className={`badge ${String(v).toLowerCase()}`}>
                {String(v)}
              </span>
            );
          return (
            <span
              className={
                p.fields[key]?.type === "decimal"
                  ? "numeric"
                  : key === "id" || key === "code"
                    ? "key-cell"
                    : ""
              }
            >
              {String(v)}
            </span>
          );
        },
      })),
    [p.columns, p.fields],
  );
  const table = useReactTable({
    data: p.rows,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualSorting: true,
    columnResizeMode: "onChange",
    state: { columnSizing: p.widths || {} },
    onColumnSizingChange: (updater) =>
      p.onWidths?.(
        typeof updater === "function" ? updater(p.widths || {}) : updater,
      ),
  });
  const rows = table.getRowModel().rows;
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => ref.current,
    estimateSize: () => 48,
    overscan: 8,
  });
  const size = table.getTotalSize();
  return (
    <div className="grid-shell" aria-busy={p.loading}>
      <div
        ref={ref}
        className="grid-scroll"
        role="grid"
        aria-label="Records"
        aria-rowcount={p.rows.length + 1}
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            const r = p.rows.find((r) => r.id === p.selectedId);
            if (r) p.onOpen(r);
          }
          if (["ArrowDown", "ArrowUp"].includes(e.key)) {
            e.preventDefault();
            const i = p.rows.findIndex((r) => r.id === p.selectedId);
            const next = Math.max(
              0,
              Math.min(p.rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)),
            );
            if (p.rows[next]) {
              p.onSelect?.(p.rows[next]);
              virtual.scrollToIndex(next);
            }
          }
        }}
      >
        <div className="grid-head" role="row" style={{ minWidth: size }}>
          {table.getHeaderGroups()[0].headers.map((h) => (
            <div
              key={h.id}
              role="columnheader"
              aria-sort={
                p.sort === h.id
                  ? p.direction === "desc"
                    ? "descending"
                    : "ascending"
                  : "none"
              }
              style={{ width: h.getSize() }}
            >
              <button onClick={() => p.onSort?.(h.id)} className="sort-button">
                {flexRender(h.column.columnDef.header, h.getContext())}
                {p.sort === h.id ? (
                  p.direction === "desc" ? (
                    <ArrowDown size={13} />
                  ) : (
                    <ArrowUp size={13} />
                  )
                ) : (
                  <ChevronsUpDown size={13} />
                )}
              </button>
              <div
                className="resizer"
                onMouseDown={h.getResizeHandler()}
                onTouchStart={h.getResizeHandler()}
              />
            </div>
          ))}
        </div>
        {rows.length === 0 ? (
          <div className="empty">
            <FileSpreadsheet size={28} />
            <strong>
              {p.loading ? "Loading cached records…" : "No records to show"}
            </strong>
            <span>Try another search or check your source status.</span>
          </div>
        ) : (
          <div
            style={{
              height: virtual.getTotalSize(),
              position: "relative",
              minWidth: size,
            }}
          >
            {virtual.getVirtualItems().map((v) => {
              const row = rows[v.index];
              return (
                <div
                  role="row"
                  aria-rowindex={v.index + 2}
                  aria-selected={row.original.id === p.selectedId}
                  key={row.id}
                  className={`data-row ${row.original.id === p.selectedId ? "selected" : ""}`}
                  style={{
                    position: "absolute",
                    top: 0,
                    transform: `translateY(${v.start}px)`,
                    height: v.size,
                    width: "100%",
                  }}
                  onClick={() => p.onSelect?.(row.original)}
                  onDoubleClick={() => p.onOpen(row.original)}
                >
                  {row.getVisibleCells().map((c) => (
                    <div
                      role="gridcell"
                      key={c.id}
                      style={{ width: c.column.getSize() }}
                      title={String(c.getValue() ?? "")}
                    >
                      {flexRender(c.column.columnDef.cell, c.getContext())}
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

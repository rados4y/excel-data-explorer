import { useEffect, useState } from "react";
import { api, label } from "./api";
import type {
  Config,
  DataRecord,
  Page,
  PageDefinition,
  RelatedView,
} from "./types";

export function NestedRecords(p: {
  config: Config;
  parentPageId: string;
  recordId: string;
  link: RelatedView;
  child: PageDefinition;
  onOpen: (record: DataRecord, pageId: string) => void;
}) {
  const [data, setData] = useState<Page | null>(null),
    [offset, setOffset] = useState(0),
    [error, setError] = useState("");
  const source = p.config.sources.find((s) => s.id === p.child.source_id)!;
  useEffect(() => {
    let active = true;
    setData(null);
    setError("");
    api("get_nested_records", {
      workspace_id: p.config.workspace.id,
      parent_page_id: p.parentPageId,
      view_id: p.link.id,
      record_id: p.recordId,
      options: { offset, limit: 25 },
    })
      .then((page) => {
        if (active) setData(page);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [p.config, p.parentPageId, p.recordId, p.link.id, offset]);
  return (
    <section className="nested-records" aria-label={`Related ${p.link.title}`}>
      <h3>
        {p.link.title}
        {data && <span className="nested-count">{data.total}</span>}
      </h3>
      {error ? (
        <p role="alert" className="inline-error">
          {error}
        </p>
      ) : !data ? (
        <p className="muted">Loading related records…</p>
      ) : !data.total ? (
        <p className="muted">No related records.</p>
      ) : (
        <>
          <div className="nested-table-scroll">
            <table className="nested-table">
              <thead>
                <tr>
                  {p.child.columns.map((f) => (
                    <th key={f}>{source.fields[f]?.label || label(f)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.records.map((r) => (
                  <tr
                    key={r.id}
                    tabIndex={0}
                    role="button"
                    aria-label={`Open related ${p.link.title} ${String(r.fields[p.child.columns[0]] ?? r.business_key)}`}
                    onClick={() => p.onOpen(r, p.child.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        p.onOpen(r, p.child.id);
                      }
                    }}
                  >
                    {p.child.columns.map((f) => (
                      <td key={f}>
                        {r.fields[f] == null ? "—" : String(r.fields[f])}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {data.total > 25 && (
            <div className="pager">
              <button
                aria-label={`Previous ${p.link.title}`}
                disabled={!offset}
                onClick={() => setOffset((v) => v - 25)}
              >
                Previous
              </button>
              <span>
                {offset + 1}–{Math.min(offset + 25, data.total)} of {data.total}
              </span>
              <button
                aria-label={`Next ${p.link.title}`}
                disabled={offset + 25 >= data.total}
                onClick={() => setOffset((v) => v + 25)}
              >
                Next
              </button>
            </div>
          )}
        </>
      )}
    </section>
  );
}

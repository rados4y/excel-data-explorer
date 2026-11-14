import { useEffect, useState } from "react";
import {
  ArrowLeft,
  ChevronRight,
  ExternalLink,
  FileText,
  X,
  AlertTriangle,
} from "lucide-react";
import { api, formatTime, label } from "./api";
import type { Config, DataRecord, Document, Page } from "./types";
interface Props {
  recordId: string;
  config: Config;
  revision: number;
  visibleColumns?: string[];
  onClose: () => void;
  onError: (s: string) => void;
}
export function Details(p: Props) {
  const [trail, setTrail] = useState<string[]>([p.recordId]);
  const id = trail.at(-1)!;
  const [record, setRecord] = useState<DataRecord | null>(null),
    [tab, setTab] = useState("Overview"),
    [related, setRelated] = useState<Page | null>(null),
    [docs, setDocs] = useState<Document[]>([]),
    [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    setTrail([p.recordId]);
    setTab("Overview");
  }, [p.recordId]);
  useEffect(() => {
    let active = true;
    setLoading(true);
    api("get_record", { record_id: id })
      .then((r) => {
        if (active) {
          setRecord(r);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (active) {
          p.onError(String(e));
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [id, p.revision]);
  const main = record?.entity_type === p.config.views.main.entity;
  const source = p.config.sources.find((s) => s.id === record?.source_id);
  const tabs: Config["views"]["detail"]["tabs"] = main
    ? p.config.views.detail.tabs
    : [
        { title: "Overview", type: "fields" },
        { title: "Documents", type: "related_documents" },
      ];
  const current = tabs.find((t) => t.title === tab) || tabs[0];
  useEffect(() => {
    let active = true;
    setRelated(null);
    setDocs([]);
    if (!record) return;
    if (
      current?.type === "relationship" &&
      "relationship" in current &&
      current.relationship
    )
      api("get_related_records", {
        record_id: id,
        relationship_id: current.relationship,
        options: { offset, limit: 50 },
      })
        .then((r) => {
          if (active) setRelated(r);
        })
        .catch((e) => p.onError(String(e)));
    if (current?.type === "related_documents")
      api("get_record_documents", { record_id: id, include_related: true })
        .then((r) => {
          if (active) setDocs(r);
        })
        .catch((e) => p.onError(String(e)));
    return () => {
      active = false;
    };
  }, [id, record?.id, tab, offset, p.revision]);
  return (
    <aside className="detail-panel" aria-label="Record details">
      <div className="detail-top">
        <span className="eyebrow">RECORD DETAILS</span>
        <button
          aria-label="Close details"
          className="icon-button"
          onClick={p.onClose}
        >
          <X size={18} />
        </button>
      </div>
      {trail.length > 1 && (
        <button
          className="back-link"
          onClick={() => {
            setTrail((t) => t.slice(0, -1));
            setTab("Overview");
            setOffset(0);
          }}
        >
          <ArrowLeft size={14} /> Back to parent record
        </button>
      )}
      <div className="detail-title">
        <div className="record-icon">
          <FileText size={22} />
        </div>
        <div>
          <h2>
            {main && record
              ? String(
                  record.fields[p.config.views.detail.title_field] ??
                    record.business_key,
                )
              : record?.business_key || "Loading…"}
          </h2>
          <p>
            {p.config.managed && source ? source.sheet : record?.entity_type}{" "}
            <ChevronRight size={12} />{" "}
            {p.config.managed && source
              ? source.path.split(/[\\/]/).at(-1)
              : record?.namespace}
          </p>
        </div>
      </div>
      <div className="tabs" role="tablist">
        {tabs.map((t) => (
          <button
            role="tab"
            aria-selected={current?.title === t.title}
            key={t.title}
            onClick={() => {
              setTab(t.title);
              setOffset(0);
            }}
          >
            {t.title}
          </button>
        ))}
      </div>
      <div className="detail-content">
        {loading ? (
          <p className="muted">Loading cached record…</p>
        ) : current?.type === "fields" && record ? (
          <>
            <div className="section-label">RECORD INFORMATION</div>
            <dl className="fields">
              {Object.entries(record.fields)
                .filter(
                  ([key]) =>
                    !main ||
                    !p.visibleColumns ||
                    p.visibleColumns.includes(key),
                )
                .map(([key, v]) => (
                  <div key={key}>
                    <dt>
                      {p.config.sources.find(
                        (s) => s.entity === record.entity_type,
                      )?.fields[key]?.label || label(key)}
                    </dt>
                    <dd>{v == null ? "—" : String(v)}</dd>
                  </div>
                ))}
            </dl>
            <details className="provenance">
              <summary>Source details</summary>
              <dl>
                <dt>Source</dt>
                <dd>{record.source_id}</dd>
                <dt>Worksheet / row</dt>
                <dd>
                  {record.source_sheet} / {record.source_row}
                </dd>
                <dt>Last changed</dt>
                <dd>{formatTime(record.updated_at)}</dd>
              </dl>
            </details>
          </>
        ) : current?.type === "relationship" ? (
          <>
            <p className="section-label">
              {related ? `${related.total} RELATED RECORDS` : "LOADING…"}
            </p>
            {related?.records.length === 0 && (
              <p className="muted">No related records.</p>
            )}
            {related?.records.map((r) => (
              <button
                className="related-card"
                key={r.id}
                onDoubleClick={() => {
                  setTrail((t) => [...t, r.id]);
                  setTab("Overview");
                  setOffset(0);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    setTrail((t) => [...t, r.id]);
                    setTab("Overview");
                    setOffset(0);
                  }
                }}
              >
                <div className="related-heading">
                  <FileText size={17} />
                  <strong>{r.business_key}</strong>
                  <ChevronRight size={16} />
                </div>
                <div className="related-values">
                  {Object.entries(r.fields)
                    .filter(
                      ([k]) => !["id", "offer_id", "document_path"].includes(k),
                    )
                    .slice(0, 3)
                    .map(([k, v]) => (
                      <span key={k}>
                        {label(k)}: {String(v ?? "—")}
                      </span>
                    ))}
                </div>
                <small>Double-click to open record</small>
              </button>
            ))}
            {related && related.total > 50 && (
              <div className="pager">
                <button
                  disabled={offset === 0}
                  onClick={() => setOffset((o) => o - 50)}
                >
                  Previous
                </button>
                <span>
                  {offset + 1}–{Math.min(offset + 50, related.total)}
                </span>
                <button
                  disabled={offset + 50 >= related.total}
                  onClick={() => setOffset((o) => o + 50)}
                >
                  Next
                </button>
              </div>
            )}
          </>
        ) : (
          <>
            <p className="section-label">
              LINKED DOCUMENTS · {docs.length}
              {docs.length === 1000 ? " (first 1,000)" : ""}
            </p>
            {docs.length === 0 && <p className="muted">No linked documents.</p>}
            {docs.map((d) => (
              <div
                className="document-card"
                key={d.id}
                onDoubleClick={() =>
                  api("open_document", { document_id: d.id }).catch((e) =>
                    p.onError(String(e)),
                  )
                }
              >
                <FileText size={24} />
                <div>
                  <strong>{d.name}</strong>
                  <span>
                    {d.document_type.toUpperCase()} · {d.mapping_id}
                  </span>
                  <span
                    className={
                      d.availability_status === "available"
                        ? "positive"
                        : "warning"
                    }
                  >
                    {d.availability_status === "available" ? (
                      "Available"
                    ) : (
                      <>
                        <AlertTriangle size={12} /> {d.availability_status}
                      </>
                    )}
                  </span>
                  {d.status_detail && <small>{d.status_detail}</small>}
                </div>
                <button
                  className="icon-button"
                  title="Open document"
                  aria-label={`Open ${d.name}`}
                  onClick={() =>
                    api("open_document", { document_id: d.id }).catch((e) =>
                      p.onError(String(e)),
                    )
                  }
                >
                  <ExternalLink size={16} />
                </button>
              </div>
            ))}
          </>
        )}
      </div>
      <div className="detail-footer">
        <span className="status-dot" /> Browsing the local cache
      </div>
    </aside>
  );
}

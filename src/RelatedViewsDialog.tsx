import { useState } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { api, label } from "./api";
import { Dialog } from "./SpaceDialogs";
import { sourceColumns } from "./spaceModel";
import type { Config, NestedViewInput, PageDefinition, Source } from "./types";

export function RelatedViewsDialog(p: {
  config: Config;
  page: PageDefinition;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const parent = p.config.sources.find((s) => s.id === p.page.source_id)!;
  const children = (p.config.pages || []).filter(
    (v) => v.source_id && v.source_id !== parent.id,
  );
  const [links, setLinks] = useState<NestedViewInput[]>(() =>
    (p.page.related_views || []).map((v) => {
      const r = p.config.relationships.find((r) => r.id === v.relationship)!;
      return {
        id: v.id,
        title: v.title,
        page_id: v.page_id,
        parent_field: r.parent_field,
        child_field: r.child_field,
      };
    }),
  );
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  function source(pageId: string) {
    return p.config.sources.find(
      (s) => s.id === children.find((v) => v.id === pageId)?.source_id,
    )!;
  }
  function initial(child: Source, parentField: string) {
    return (
      sourceColumns(child).find(
        (f) => child.fields[f].column === parent.fields[parentField]?.column,
      ) ||
      sourceColumns(child)[0] ||
      ""
    );
  }
  function update(i: number, value: Partial<NestedViewInput>) {
    setLinks((rows) => rows.map((v, j) => (j === i ? { ...v, ...value } : v)));
  }
  return (
    <Dialog
      title={`Related views for ${p.page.title}`}
      description="Choose the child views and matching fields. Selecting a parent row shows all its related lists below the record details."
      onClose={p.onClose}
      wide
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          try {
            await api("save_nested_views", {
              workspace_id: p.config.workspace.id,
              page_id: p.page.id,
              views: links,
            });
            await p.onSaved();
            p.onClose();
          } catch (e) {
            setError(String(e));
            setBusy(false);
          }
        }}
      >
        <p className="small muted">
          Match a business identifier such as Offers.OfferID to
          Invoices.OfferID. Values and field types must match exactly. Each link
          is limited to the two selected worksheets in this space; record
          identities stay unchanged. Child views retain their visible columns,
          saved filters and sorting, including when hidden from the sidebar.
        </p>
        {!children.length && (
          <p>
            Add another worksheet to this space before creating a related view.
          </p>
        )}
        {links.map((v, i) => {
          const child = source(v.page_id);
          const mismatch =
            parent.fields[v.parent_field]?.type !==
            child?.fields[v.child_field]?.type;
          return (
            <fieldset
              className="nested-link-editor"
              key={v.id || i}
              disabled={busy}
            >
              <legend>Related list {i + 1}</legend>
              <div className="form-two-columns">
                <label className="form-field">
                  List title
                  <input
                    aria-label={`Related list title ${i + 1}`}
                    required
                    maxLength={200}
                    value={v.title}
                    onChange={(e) => update(i, { title: e.target.value })}
                  />
                </label>
                <label className="form-field">
                  Child view
                  <select
                    aria-label={`Child view ${i + 1}`}
                    value={v.page_id}
                    onChange={(e) => {
                      const next = children.find(
                        (p) => p.id === e.target.value,
                      )!;
                      update(i, {
                        page_id: next.id,
                        title: next.title,
                        child_field: initial(source(next.id), v.parent_field),
                      });
                    }}
                  >
                    {children.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.title}
                        {c.hidden ? " (hidden in sidebar)" : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="form-field">
                  Parent matching field
                  <select
                    aria-label={`Parent matching field ${i + 1}`}
                    required
                    value={v.parent_field}
                    onChange={(e) =>
                      update(i, { parent_field: e.target.value })
                    }
                  >
                    {sourceColumns(parent).map((f) => (
                      <option key={f} value={f}>
                        {parent.fields[f].label || label(f)} (
                        {parent.fields[f].type})
                      </option>
                    ))}
                  </select>
                </label>
                <label className="form-field">
                  Child matching field
                  <select
                    aria-label={`Child matching field ${i + 1}`}
                    required
                    value={v.child_field}
                    onChange={(e) => update(i, { child_field: e.target.value })}
                  >
                    <option value="" disabled>
                      Choose a field
                    </option>
                    {sourceColumns(child).map((f) => (
                      <option key={f} value={f}>
                        {child.fields[f].label || label(f)} (
                        {child.fields[f].type})
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              {mismatch && (
                <p className="inline-error">
                  Field types differ. Choose matching fields or update the
                  worksheet types before saving.
                </p>
              )}
              <div className="nested-link-actions">
                <button
                  type="button"
                  className="icon-button"
                  aria-label={`Move related list ${i + 1} up`}
                  disabled={i === 0}
                  onClick={() =>
                    setLinks((a) => {
                      const n = [...a];
                      [n[i - 1], n[i]] = [n[i], n[i - 1]];
                      return n;
                    })
                  }
                >
                  <ArrowUp size={16} />
                </button>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={`Move related list ${i + 1} down`}
                  disabled={i === links.length - 1}
                  onClick={() =>
                    setLinks((a) => {
                      const n = [...a];
                      [n[i + 1], n[i]] = [n[i], n[i + 1]];
                      return n;
                    })
                  }
                >
                  <ArrowDown size={16} />
                </button>
                <button
                  type="button"
                  className="button"
                  onClick={() => setLinks((a) => a.filter((_, j) => i !== j))}
                >
                  <Trash2 size={14} />
                  Remove list {i + 1}
                </button>
              </div>
            </fieldset>
          );
        })}
        <button
          type="button"
          className="button"
          disabled={
            busy ||
            !children.length ||
            links.length >= 20 ||
            !sourceColumns(parent).length
          }
          onClick={() => {
            const child = children[0],
              field =
                parent.key !== "__row_id"
                  ? parent.key
                  : sourceColumns(parent)[0];
            setLinks((a) => [
              ...a,
              {
                id: null,
                title: child.title,
                page_id: child.id,
                parent_field: field,
                child_field: initial(source(child.id), field),
              },
            ]);
          }}
        >
          <Plus size={15} />
          Add related list
        </button>
        {error && (
          <p role="alert" className="inline-error">
            {error}
          </p>
        )}
        <div className="form-actions">
          <button type="button" className="button" onClick={p.onClose}>
            Cancel
          </button>
          <button
            className="button primary"
            disabled={
              busy ||
              links.some(
                (v) =>
                  !v.title.trim() ||
                  !v.child_field ||
                  parent.fields[v.parent_field]?.type !==
                    source(v.page_id)?.fields[v.child_field]?.type,
              )
            }
          >
            {busy ? "Saving…" : "Save related views"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

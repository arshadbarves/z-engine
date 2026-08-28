import { useEffect, useState } from "react";
import { listPermissionRules, removePermissionRule, savePermissionRule } from "../../lib/commands";

export function PermissionsTab() {
  const [rules, setRules] = useState<string[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    listPermissionRules()
      .then(setRules)
      .catch(() => setRules([]));
  }, []);

  async function add() {
    const rule = draft.trim();
    if (!rule) return;
    await savePermissionRule(rule);
    setDraft("");
    setRules(await listPermissionRules());
  }

  async function remove(rule: string) {
    await removePermissionRule(rule);
    setRules(await listPermissionRules());
  }

  return (
    <div className="tab-body">
      <section className="settings-group">
        <h3>Allow rules</h3>
        <p className="form-note">Bash prefixes that skip approval.</p>
        <ul className="rule-list">
        {rules.map((r) => (
          <li key={r}>
            <code>{r}</code>
            <button className="mini" title={`Remove ${r}`} onClick={() => void remove(r)}>
              ✕
            </button>
          </li>
        ))}
        {rules.length === 0 && <li className="none">No rules yet.</li>}
      </ul>
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          value={draft}
          onChange={(e) => setDraft(e.currentTarget.value)}
          placeholder='e.g. "cargo test*"'
          spellCheck={false}
        />
        <button type="submit" disabled={!draft.trim()}>
          Add rule
        </button>
      </form>
      </section>
    </div>
  );
}

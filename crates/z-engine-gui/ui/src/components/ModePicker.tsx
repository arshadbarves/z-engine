import { useState, useSyncExternalStore } from "react";
import { ChevronDown, Shield } from "../lib/icons";
import { modeStore } from "../lib/events";
import { setMode } from "../lib/commands";

const MODES = [
  {
    id: "normal",
    label: "normal",
    desc: "Ask before every file edit and command",
  },
  {
    id: "accept-edits",
    label: "auto-accept edits",
    desc: "Apply edits without asking; commands still gated",
  },
  {
    id: "plan",
    label: "plan",
    desc: "Read-only — explore and propose, change nothing",
  },
] as const;

/** Permission-mode picker in the ModelPicker popover style (replaces the
 * native <select> so both chips share one custom dropdown language). */
export function ModePicker() {
  const mode = useSyncExternalStore(modeStore.subscribe, () => modeStore.getSnapshot());
  const [open, setOpen] = useState(false);
  const current = MODES.find((m) => m.id === mode) ?? MODES[0];

  async function pick(id: string) {
    setOpen(false);
    if (id === mode) return;
    modeStore.set(id);
    try {
      await setMode(id);
    } catch (e) {
      console.error(e);
    }
  }

  return (
    <div className="model-picker">
      {open && <div className="popover-backdrop" onClick={() => setOpen(false)} />}
      <button className="mode model-btn" onClick={() => setOpen((o) => !o)} title="Permission mode">
        <Shield size={11} />
        <span>{current.label}</span>
        <ChevronDown size={9} strokeWidth={2.4} />
      </button>
      {open && (
        <div className="popover" role="menu">
          <div className="popover-head">Permission mode</div>
          <div className="popover-current">{current.label}</div>
          {MODES.filter((m) => m.id !== mode).map((m) => (
            <button key={m.id} className="popover-item" role="menuitem" onClick={() => void pick(m.id)}>
              {m.label}
              <span className="popover-sub">{m.desc}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}


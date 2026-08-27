type Listener = () => void;

export interface ShellEntry {
  id: number;
  cmd: string;
  lines: string[];
}

export interface ShellState {
  visible: boolean;
  entries: ShellEntry[];
}

const listeners = new Set<Listener>();
let nextId = 1;
let state: ShellState = { visible: false, entries: [] };

function emit() {
  for (const l of listeners) l();
}

export const shellStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): ShellState {
    return state;
  },
};

/** Open the overlay and start a new `!` command block. */
export function startShell(cmd: string) {
  state = {
    visible: true,
    entries: [...state.entries.slice(-19), { id: nextId++, cmd, lines: [] }],
  };
  emit();
}

/** Append a stdout line (core sends `$ line` status notes). */
export function appendShellLine(raw: string) {
  const text = raw.startsWith("$ ") ? raw.slice(2) : raw;
  const entries = state.entries.slice();
  const last = entries[entries.length - 1];
  if (!last) {
    entries.push({ id: nextId++, cmd: "", lines: [text] });
  } else {
    entries[entries.length - 1] = { ...last, lines: [...last.lines, text] };
  }
  state = { visible: true, entries };
  emit();
}

export function hideShell() {
  if (!state.visible) return;
  state = { ...state, visible: false };
  emit();
}

export function showShell() {
  if (state.visible || state.entries.length === 0) return;
  state = { ...state, visible: true };
  emit();
}

export function resetShell() {
  nextId = 1;
  state = { visible: false, entries: [] };
  emit();
}

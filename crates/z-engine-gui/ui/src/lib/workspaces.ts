import {
  addWorkspace as invokeAdd,
  listWorkspaces,
  removeWorkspace as invokeRemove,
} from "./commands";

/** Registered workspace roots (Codex-desktop style projects) plus the
 * active one new tasks run against. Persisted backend-side in
 * `z-engine/workspaces.json`; this store mirrors it for the UI. */
let roots: string[] = [];
let active: string | null = null;

type Listener = () => void;
const subs = new Set<Listener>();
/** Cached snapshot: useSyncExternalStore requires a stable identity
 * between emits — returning a fresh object per call loops forever. */
let snapshot: { roots: string[]; active: string | null } = { roots, active };

function emit() {
  snapshot = { roots, active };
  for (const l of subs) l();
}

export const workspaceStore = {
  subscribe(l: Listener) {
    subs.add(l);
    return () => {
      subs.delete(l);
    };
  },
  getSnapshot(): { roots: string[]; active: string | null } {
    return snapshot;
  },
  /** Fetch the persisted roots; default active to the first entry. */
  async load() {
    try {
      roots = await listWorkspaces();
      if (active === null && roots.length > 0) active = roots[0];
      emit();
    } catch (e) {
      console.error(e);
    }
  },
  setActive(root: string | null) {
    if (root === active) return;
    active = root;
    emit();
  },
  async add(path: string) {
    const canonical = await invokeAdd(path);
    await workspaceStore.load();
    // Make the freshly added root active so "New task" targets it.
    workspaceStore.setActive(canonical);
  },
  async remove(path: string) {
    if (active === path) active = null;
    await invokeRemove(path);
    await workspaceStore.load();
    if (active === null && roots.length > 0) workspaceStore.setActive(roots[0]);
  },
};

export function wsBasename(root: string): string {
  const parts = root.replace(/\/+$/, "").split("/");
  return parts[parts.length - 1] || root;
}

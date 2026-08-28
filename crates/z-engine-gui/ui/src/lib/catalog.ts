import { fetchModelCatalog } from "./commands";

/** Trimmed models.dev entry (plus local models.json overrides). */
export interface CatalogModel {
  name: string;
  reasoning: boolean;
  attachment: boolean;
  context?: number;
  output?: number;
}

export interface CatalogProvider {
  name: string;
  models: Record<string, CatalogModel>;
}

export interface CatalogData {
  [providerId: string]: CatalogProvider;
}

/** Provider id shown in the model picker (multi-provider lands later). */
export const PICKER_PROVIDER_ID = "openrouter";

/** Restrict the catalog to OpenRouter so the picker only lists that provider. */
export function catalogForPicker(catalog: CatalogData | null): CatalogData {
  if (!catalog) return {};
  const prov = catalog[PICKER_PROVIDER_ID];
  if (!prov) return {};
  return { [PICKER_PROVIDER_ID]: prov };
}

let data: CatalogData | null = null;
let loading: Promise<void> | null = null;
/** Set when the last fetch failed; `ensure()` retries on next call. */
let failed = false;
type Listener = () => void;
const subs = new Set<Listener>();

function emit() {
  for (const l of subs) l();
}

export const catalogStore = {
  subscribe(l: Listener) {
    subs.add(l);
    return () => {
      subs.delete(l);
    };
  },
  getSnapshot(): CatalogData | null {
    return data;
  },
  getFailed(): boolean {
    return failed;
  },
  /** Fetch once; safe to call on every picker open. A failed fetch does
   * not poison the cache — the next open retries (offline-at-launch
   * users get the picker as soon as the network is back). */
  async ensure() {
    if (data || loading) {
      await loading;
      return;
    }
    const done = (async () => {
      try {
        data = await fetchModelCatalog();
        failed = false;
        emit();
      } catch (e) {
        failed = true;
        console.error("model catalog unavailable:", e);
      } finally {
        loading = null;
      }
    })();
    loading = done;
    await done;
  },
};

/** Find a model entry for ids like "anthropic/claude-sonnet-4" — tries the
 * full id, then suffix matches after each slash, across all providers. */
export function lookupModel(
  catalog: CatalogData | null,
  modelId: string,
): { providerId: string; id: string; model: CatalogModel } | null {
  if (!catalog || !modelId) return null;
  const parts = modelId.split("/");
  const candidates = [modelId];
  for (let i = 1; i < parts.length; i++) candidates.push(parts.slice(i).join("/"));
  for (const candidate of candidates) {
    for (const [pid, prov] of Object.entries(catalog)) {
      if (prov.models[candidate]) {
        return { providerId: pid, id: candidate, model: prov.models[candidate] };
      }
    }
  }
  return null;
}

export function fmtLimit(n?: number): string {
  if (!n) return "";
  return n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);
}

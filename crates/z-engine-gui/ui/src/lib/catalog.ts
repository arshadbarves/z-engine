import { fetchModelCatalog } from "./commands";
import type { ModelInfo } from "./protocol/ModelInfo";
import { detectProviderId, PROVIDERS, requiresApiKey } from "./providers";

/** The model catalog (models.dev plus local overrides), in lookup-preference order. */
export type CatalogData = ModelInfo[];

export interface ModelGroup {
  provider: string;
  items: ModelInfo[];
}

/** Restrict the picker to the active provider and hide keyed providers after
 * their credential is disconnected. */
export function catalogForPicker(
  catalog: CatalogData | null,
  baseUrl: string | null | undefined,
  hasApiKey: boolean,
): CatalogData {
  if (!catalog) return [];
  const preset = PROVIDERS.find((provider) => provider.id === detectProviderId(baseUrl));
  if (!preset || (requiresApiKey(preset) && !hasApiKey)) return [];
  return catalog.filter((model) => model.provider === preset.catalogProvider);
}

function providerName(id: string): string {
  return PROVIDERS.find((provider) => provider.catalogProvider === id)?.name ?? id;
}

/** Models matching `query` by id, name or provider, grouped by provider, at most `limit` per group. */
export function groupModels(models: CatalogData, query: string, limit = 40): ModelGroup[] {
  const q = query.trim().toLowerCase();
  const groups = new Map<string, ModelInfo[]>();
  for (const model of models) {
    const name = providerName(model.provider);
    const hit = !q || [model.id, model.name, name].some((text) => text.toLowerCase().includes(q));
    if (!hit) continue;
    const items = groups.get(name) ?? [];
    if (items.length < limit) items.push(model);
    groups.set(name, items);
  }
  return [...groups]
    .map(([provider, items]) => ({ provider, items }))
    .sort((a, b) => a.provider.localeCompare(b.provider));
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

function unprefixed(id: string): string {
  const slash = id.indexOf("/");
  return slash >= 0 ? id.slice(slash + 1) : id;
}

/** Find a model like the engine does: exact id, then ignoring a `vendor/`
 * prefix on either side, then both again ignoring case. */
export function lookupModel(
  catalog: CatalogData | null,
  modelId: string,
): { providerId: string; id: string; model: ModelInfo } | null {
  const wanted = modelId.trim();
  if (!catalog || !wanted) return null;
  const bare = unprefixed(wanted);
  const lower = (text: string) => text.toLowerCase();
  const tests: Array<(id: string) => boolean> = [
    (id) => id === wanted,
    (id) => unprefixed(id) === bare,
    (id) => lower(id) === lower(wanted),
    (id) => lower(unprefixed(id)) === lower(bare),
  ];
  for (const test of tests) {
    const model = catalog.find((entry) => test(entry.id));
    if (model) return { providerId: model.provider, id: model.id, model };
  }
  return null;
}

export function fmtLimit(n?: number): string {
  if (!n) return "";
  return n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);
}

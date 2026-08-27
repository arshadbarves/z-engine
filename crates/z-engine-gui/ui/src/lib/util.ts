export interface SessionEntry {
  path: string;
  ulid: string;
  firstUserMsg: string | null;
  modifiedMs: number;
  projectRoot?: string | null;
  unreadOutcome?: string | null;
}

export interface SessionGroup {
  label: string;
  items: SessionEntry[];
}

const DAY = 86_400_000;

function startOfDay(t: number): number {
  const d = new Date(t);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Bucket label for a session timestamp: Today / Yesterday / `Aug 12`. */
export function dayBucket(ms: number, now = Date.now()): string {
  const today = startOfDay(now);
  const day = startOfDay(ms);
  if (day === today) return "Today";
  if (day === today - DAY) return "Yesterday";
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** Group newest-first sessions into day buckets, preserving recency order. */
export function groupSessions(
  list: SessionEntry[],
  now = Date.now(),
): SessionGroup[] {
  const groups: SessionGroup[] = [];
  for (const s of list) {
    const label = dayBucket(Number(s.modifiedMs), now);
    const g = groups.find((x) => x.label === label);
    if (g) g.items.push(s);
    else groups.push({ label, items: [s] });
  }
  return groups;
}

/** Case-insensitive filter over preview text and ULID prefix. */
export function filterSessions(
  list: SessionEntry[],
  query: string,
): SessionEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (s) =>
      (s.firstUserMsg ?? "").toLowerCase().includes(q) ||
      s.ulid.toLowerCase().includes(q),
  );
}

export function relTime(ms: number, now = Date.now()): string {
  const d = now - ms;
  if (d < 60_000) return "now";
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`;
  if (d < DAY) return `${Math.floor(d / 3_600_000)}h`;
  return `${Math.floor(d / DAY)}d`;
}

export function shortModel(id: string): string {
  const slash = id.lastIndexOf("/");
  return slash >= 0 ? id.slice(slash + 1) : id;
}

/** `1234` → `"1.2k"`; below 1000 stays plain. */
export function fmtTokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(2)}M`;
}

export interface Pricing {
  usdPerMtokInput: number;
  usdPerMtokOutput: number;
}

/** Session cost estimate; null when pricing is unknown for the model. */
export function estimateCost(
  pricing: Pricing | null | undefined,
  promptTokens: number,
  completionTokens: number,
): number | null {
  if (!pricing) return null;
  return (
    (promptTokens / 1_000_000) * pricing.usdPerMtokInput +
    (completionTokens / 1_000_000) * pricing.usdPerMtokOutput
  );
}

export function fmtCost(usd: number | null): string {
  if (usd == null) return "–";
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

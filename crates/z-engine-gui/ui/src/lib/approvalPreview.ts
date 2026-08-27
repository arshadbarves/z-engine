/** Pretty-print an approval card without dumping the raw JSON preview. */

export function approvalToolName(m: {
  toolName?: string;
  text: string;
}): string {
  if (m.toolName) return m.toolName;
  const body = m.text.replace(/^⚠ approval required — /, "");
  const nl = body.indexOf("\n");
  return (nl >= 0 ? body.slice(0, nl) : body).trim();
}

export function approvalCommand(m: {
  bashCommand?: string | null;
  text: string;
}): string {
  if (m.bashCommand) return m.bashCommand;
  const body = m.text.replace(/^⚠ approval required — /, "");
  const nl = body.indexOf("\n");
  const detail = nl >= 0 ? body.slice(nl + 1) : "";
  const raw = detail.replace(/^input:\s*/, "").trim();
  if (!raw) return "";
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    if (typeof v.command === "string") return v.command;
    if (typeof v.path === "string") return v.path;
    if (typeof v.pattern === "string") return v.pattern;
    if (typeof v.query === "string") return v.query;
  } catch {
    /* not JSON */
  }
  return raw;
}

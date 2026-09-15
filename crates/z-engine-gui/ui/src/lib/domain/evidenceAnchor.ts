/** Anchor ids are used as both DOM ids and URL fragments, so only
 *  `[A-Za-z0-9_]` may survive. Unsafe characters — including `_` itself —
 *  become `_<code point in hex>_`, which keeps the mapping injective: two
 *  different evidence ids can never produce the same anchor. */
export function safeAnchorSlug(value: string): string {
  let slug = "";
  for (const char of value) {
    slug += /^[A-Za-z0-9]$/.test(char)
      ? char
      : `_${(char.codePointAt(0) ?? 0).toString(16)}_`;
  }
  return slug;
}

export function evidenceAnchorId(prefix: string, evidenceId: string): string {
  return `${prefix}-${safeAnchorSlug(evidenceId)}`;
}

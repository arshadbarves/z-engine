import type { QueuedMessage } from "../types";

const PREVIEW_CHARS = 48;

/** One-line label for a queued follow-up so the composer queue row never wraps. */
export function queuePreview(item: QueuedMessage): string {
  const text = item.text.replace(/\s*[\r\n]+\s*/g, " ").trim();
  if (text) {
    return text.length > PREVIEW_CHARS ? `${text.slice(0, PREVIEW_CHARS - 1)}…` : text;
  }
  const count = item.images.length;
  if (count > 0) return `${count} ${count === 1 ? "image" : "images"}`;
  return "Empty follow-up";
}

/** Hover text for a queued follow-up: the full prompt, never blank. */
export function queueTitle(item: QueuedMessage): string {
  return item.text.trim() || queuePreview(item);
}

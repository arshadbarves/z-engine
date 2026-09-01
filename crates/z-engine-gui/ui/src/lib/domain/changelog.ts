export interface ChangelogSection {
  title: string;
  kind: "added" | "fixed" | "changed" | "removed" | "other";
  items: string[];
}

export interface ChangelogRelease {
  version: string;
  date: string;
  isLatest: boolean;
  sections: ChangelogSection[];
}

function detectKind(title: string): ChangelogSection["kind"] {
  const lower = title.toLowerCase();
  if (lower.includes("add") || lower.includes("feature")) return "added";
  if (lower.includes("fix") || lower.includes("bug")) return "fixed";
  if (lower.includes("change") || lower.includes("improv") || lower.includes("update")) return "changed";
  if (lower.includes("remov") || lower.includes("deprecat")) return "removed";
  return "other";
}

export function parseChangelog(raw: string): ChangelogRelease[] {
  if (!raw || typeof raw !== "string") return [];
  const lines = raw.split("\n");
  const releases: ChangelogRelease[] = [];
  let currentRelease: ChangelogRelease | null = null;
  let currentSection: ChangelogSection | null = null;

  for (const line of lines) {
    const trimmed = line.trim();

    // Match ## [1.4.1] - 2026-09-01 or ## 1.4.1 (2026-09-01)
    const releaseMatch = trimmed.match(/^##\s+\[?v?([0-9]+\.[0-9]+\.[0-9]+[^\]\s]*)\]?(?:\s*-\s*|\s+\()?([0-9]{4}-[0-9]{2}-[0-9]{2})?\)?/);
    if (releaseMatch) {
      if (currentSection && currentRelease) {
        currentRelease.sections.push(currentSection);
        currentSection = null;
      }
      if (currentRelease) {
        releases.push(currentRelease);
      }
      currentRelease = {
        version: releaseMatch[1] ?? "",
        date: releaseMatch[2] ?? "",
        isLatest: releases.length === 0,
        sections: [],
      };
      continue;
    }

    // Match ### Added / ### Fixed / ### Changed
    const sectionMatch = trimmed.match(/^###\s+(.+)$/);
    if (sectionMatch && currentRelease) {
      if (currentSection) {
        currentRelease.sections.push(currentSection);
      }
      const title = sectionMatch[1]?.trim() ?? "Changes";
      currentSection = {
        title,
        kind: detectKind(title),
        items: [],
      };
      continue;
    }

    // Match list items: - item or * item
    if (trimmed.startsWith("- ") || trimmed.startsWith("* ")) {
      const itemText = trimmed.slice(2).trim();
      if (itemText && currentSection) {
        currentSection.items.push(itemText);
      } else if (itemText && currentRelease) {
        if (!currentSection) {
          currentSection = { title: "Changes", kind: "changed", items: [] };
        }
        currentSection.items.push(itemText);
      }
    }
  }

  if (currentSection && currentRelease) {
    currentRelease.sections.push(currentSection);
  }
  if (currentRelease) {
    releases.push(currentRelease);
  }

  return releases;
}

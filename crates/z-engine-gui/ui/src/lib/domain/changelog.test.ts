import { describe, expect, it } from "vitest";
import { parseChangelog } from "./changelog";

describe("parseChangelog", () => {
  it("parses empty text into empty array", () => {
    expect(parseChangelog("")).toEqual([]);
  });

  it("parses structured release with sections and bullets", () => {
    const raw = `
# Changelog

## [1.4.1] - 2026-09-01

### Fixed
- **Auto-Updater**: Fixed signature verification.
- **Cache**: Removed disk cache.

### Added
- **Changelog**: Added live changelog viewer.

## [1.4.0] - 2026-08-31

### Added
- **Workbench**: Master-Detail review layout.
`;
    const releases = parseChangelog(raw);
    expect(releases.length).toBe(2);

    expect(releases[0]?.version).toBe("1.4.1");
    expect(releases[0]?.date).toBe("2026-09-01");
    expect(releases[0]?.isLatest).toBe(true);
    expect(releases[0]?.sections.length).toBe(2);
    expect(releases[0]?.sections[0]?.kind).toBe("fixed");
    expect(releases[0]?.sections[0]?.items.length).toBe(2);

    expect(releases[1]?.version).toBe("1.4.0");
    expect(releases[1]?.isLatest).toBe(false);
  });
});

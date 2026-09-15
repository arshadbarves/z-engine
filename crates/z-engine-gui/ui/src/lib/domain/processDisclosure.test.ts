import { describe, expect, it } from "vitest";
import {
  beginProcessRun,
  endProcessRun,
  transitionProcessRun,
  toggleProcessDisclosure,
  type ProcessDisclosure,
} from "./processDisclosure";

const collapsed: ProcessDisclosure = {
  expanded: false,
  autoOpened: false,
  preRunExpanded: false,
};

describe("beginProcessRun", () => {
  it("auto-opens a collapsed process and records that it owns the open state", () => {
    expect(beginProcessRun(collapsed)).toEqual({
      expanded: true,
      autoOpened: true,
      preRunExpanded: false,
    });
  });

  it("leaves a user-expanded process alone and remembers the pre-run state", () => {
    expect(beginProcessRun({ ...collapsed, expanded: true })).toEqual({
      expanded: true,
      autoOpened: false,
      preRunExpanded: true,
    });
  });
});

describe("endProcessRun", () => {
  it("auto-closes only what streaming opened", () => {
    expect(endProcessRun(beginProcessRun(collapsed))).toEqual({
      expanded: false,
      autoOpened: false,
      preRunExpanded: false,
    });
  });

  it("preserves a pre-run expanded process", () => {
    expect(endProcessRun(beginProcessRun({ ...collapsed, expanded: true }))).toEqual({
      expanded: true,
      autoOpened: false,
      preRunExpanded: true,
    });
  });

  it("preserves a choice the user made during the run", () => {
    const opened = beginProcessRun(collapsed);
    const keptOpen = toggleProcessDisclosure(toggleProcessDisclosure(opened));
    expect(endProcessRun(keptOpen).expanded).toBe(true);
    expect(endProcessRun(toggleProcessDisclosure(opened)).expanded).toBe(false);
  });
});

describe("toggleProcessDisclosure", () => {
  it("hands ownership of the open state to the user", () => {
    expect(toggleProcessDisclosure(beginProcessRun(collapsed))).toEqual({
      expanded: false,
      autoOpened: false,
      preRunExpanded: false,
    });
    expect(toggleProcessDisclosure(collapsed)).toEqual({
      expanded: true,
      autoOpened: false,
      preRunExpanded: false,
    });
  });
});

describe("transitionProcessRun", () => {
  it("auto-opens at stream start and auto-closes at stream end", () => {
    const streaming = transitionProcessRun(collapsed, false, true);
    expect(streaming.expanded).toBe(true);
    expect(transitionProcessRun(streaming, true, false).expanded).toBe(false);
  });

  it("preserves the reader's explicit streaming choice at stream end", () => {
    const streaming = transitionProcessRun(collapsed, false, true);
    const collapsedByReader = toggleProcessDisclosure(streaming);
    expect(transitionProcessRun(collapsedByReader, true, false).expanded).toBe(false);

    const expandedByReader = toggleProcessDisclosure(collapsedByReader);
    expect(transitionProcessRun(expandedByReader, true, false).expanded).toBe(true);
  });

  it("does not re-open after the reader collapses during the same stream", () => {
    const streaming = transitionProcessRun(collapsed, false, true);
    const collapsedByReader = toggleProcessDisclosure(streaming);
    expect(transitionProcessRun(collapsedByReader, true, true).expanded).toBe(false);
  });
});

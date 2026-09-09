import { describe, expect, it } from "vitest";
import { loadGuarded, saveGuarded } from "./guardedMode";

function fakeStore() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => (data.has(k) ? data.get(k)! : null),
    setItem: (k: string, v: string) => void data.set(k, v),
  };
}

describe("guardedMode", () => {
  it("defaults to unguarded", () => {
    expect(loadGuarded(fakeStore())).toBe(false);
  });

  it("round-trips the toggle", () => {
    const store = fakeStore();
    saveGuarded(store, true);
    expect(loadGuarded(store)).toBe(true);
    saveGuarded(store, false);
    expect(loadGuarded(store)).toBe(false);
  });

  it("treats corrupt values as unguarded", () => {
    const store = fakeStore();
    store.setItem("zeng.guarded", "maybe");
    expect(loadGuarded(store)).toBe(false);
  });
});

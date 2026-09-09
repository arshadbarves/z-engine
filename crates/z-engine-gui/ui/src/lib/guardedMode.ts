export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const KEY = "zeng.guarded";

export function loadGuarded(store: StorageLike): boolean {
  return store.getItem(KEY) === "1";
}

export function saveGuarded(store: StorageLike, value: boolean): void {
  store.setItem(KEY, value ? "1" : "0");
}

export function browserGuarded(): boolean {
  if (typeof window === "undefined" || !window.localStorage) return false;
  return loadGuarded(window.localStorage);
}

export function saveBrowserGuarded(value: boolean): void {
  if (typeof window === "undefined" || !window.localStorage) return;
  saveGuarded(window.localStorage, value);
}

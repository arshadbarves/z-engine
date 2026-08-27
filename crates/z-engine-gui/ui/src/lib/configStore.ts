import type { HarnessConfig } from "./commands";

type Listener = () => void;
const subs = new Set<Listener>();

let config: HarnessConfig | null = null;

export const configStore = {
  subscribe(l: Listener) {
    subs.add(l);
    return () => {
      subs.delete(l);
    };
  },
  getSnapshot(): HarnessConfig | null {
    return config;
  },
  set(c: HarnessConfig | null) {
    config = c;
    for (const l of subs) l();
  },
};

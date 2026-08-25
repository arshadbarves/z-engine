import { invoke } from "@tauri-apps/api/core";
export { invoke };

export const submit = (text: string) => invoke("submit", { text });
export const abort = () => invoke("abort");
export const compact = () => invoke("compact");

export const approve_with_rule = (args: { id: number; decision: string; rule: string }) =>
  invoke("approve_with_rule", args);
export const denyCmd = (id: number) => invoke("deny", { id });

export const set_mode = (mode: string) => invoke("set_mode", { mode });

import { invoke } from "@tauri-apps/api/core";

export const submit = (text: string) => invoke("submit", { text });
export const abort = () => invoke("abort");
export const compact = () => invoke("compact");

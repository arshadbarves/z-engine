import { invoke } from "@tauri-apps/api/core";

export { invoke };

export const submit = (text: string) => invoke("submit", { text });
export const abort = () => invoke("abort");
export const compact = () => invoke("compact");
export const notes = () => invoke("notes");
export const setMode = (mode: string) => invoke("set_mode", { mode });
export const setModel = (model: string) => invoke("set_model", { model });
export const deny = (id: number) => invoke("deny", { id });
export const approveWithRule = (
  id: number,
  decision: "session" | "persist",
  rule: string,
) => invoke("approve_with_rule", { id, decision, rule });
export const startSession = (resumePath: string | null) =>
  invoke("start_session", { resumePath });
export const listSessions = () => invoke("list_sessions");
export const deleteSession = (path: string) => invoke("delete_session", { path });

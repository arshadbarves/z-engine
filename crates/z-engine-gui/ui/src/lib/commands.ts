import { invoke } from "@tauri-apps/api/core";

export { invoke };

export const submit = (text: string, images: string[] = [], sessionId?: string) =>
  invoke("submit", { text, images, sessionId: sessionId ?? null });

export const abort = (sessionId?: string) => invoke("abort", { sessionId: sessionId ?? null });
export const compact = () => invoke("compact");
export const notes = () => invoke("notes");
export const setMode = (mode: string) => invoke("set_mode", { mode });
export const setModel = (model: string) => invoke("set_model", { model });
export const shellPassthrough = (cmd: string) => invoke("shell", { cmd });
export const revertLastTurn = () => invoke("revert_last_turn");
export const deny = (id: number) => invoke("deny", { id });
export const approveWithRule = (
  id: number,
  decision: "once" | "session" | "persist",
  rule: string,
) => invoke("approve_with_rule", { id, decision, rule });
export interface StartSessionResult {
  ulid: string;
  events: unknown[];
  alreadyLive?: boolean;
  path?: string | null;
}

export const startSession = (resumePath: string | null, root?: string | null) =>
  invoke<StartSessionResult>("start_session", { resumePath, root });
export const listSessions = () => invoke("list_sessions");
export const deleteSession = (path: string) => invoke("delete_session", { path });

export const revertToTurn = (keep: number) => invoke("revert_to_turn", { keep });

export const listWorkspaces = () => invoke<string[]>("list_workspaces");
export const addWorkspace = (path: string) => invoke<string>("add_workspace", { path });
export const removeWorkspace = (path: string) => invoke("remove_workspace", { path });

export const fetchModelCatalog = () =>
  invoke<import("./catalog").CatalogData>("fetch_model_catalog");
export const setReasoningEffort = (effort: string | null) =>
  invoke("set_reasoning_effort", { effort });

export interface SlashCommandInfo {
  name: string;
  source: string;
  description: string;
}
export const listSlashCommands = () => invoke<SlashCommandInfo[]>("list_slash_commands");
export const readSlashCommand = (name: string) =>
  invoke<string>("read_slash_command", { name });

export interface ChangedFile {
  path: string;
  status: string;
}
export const listChangedFiles = () => invoke<ChangedFile[]>("list_changed_files");
export const diffForFile = (path: string) => invoke<string>("diff_for_file", { path });

export const createWorktree = (name: string) => invoke<string>("create_worktree", { name });
export const readSession = (path: string) =>
  invoke<unknown[]>("read_session", { path });

export interface PricingInfo {
  usdPerMtokInput: number;
  usdPerMtokOutput: number;
}

export interface McpServerInfo {
  name: string;
  command: string;
  args: string[];
}

export interface HarnessConfig {
  model: string;
  maxContextTokens: number;
  maxOutputTokens?: number;
  compactAtPercent?: number;
  baseUrl?: string;
  reviewEnabled?: boolean;
  pricing?: PricingInfo | null;
  mcpServers?: McpServerInfo[];
  costOverrides?: Record<string, PricingInfo>;
  version?: string;
  projectName?: string;
}

export const getConfig = () => invoke<HarnessConfig>("get_config");

export interface GeneralPatch {
  model?: string | null;
  baseUrl?: string | null;
  maxContextTokens?: number | null;
  review?: boolean | null;
}

export const saveGeneral = (p: GeneralPatch) =>
  invoke("save_general", {
    model: p.model ?? null,
    baseUrl: p.baseUrl ?? null,
    maxContextTokens: p.maxContextTokens ?? null,
    review: p.review ?? null,
  });

export const listPermissionRules = () =>
  invoke<string[]>("list_permission_rules");
export const savePermissionRule = (rule: string) =>
  invoke("save_permission_rule", { rule });
export const removePermissionRule = (rule: string) =>
  invoke("remove_permission_rule", { rule });

export const listMcpServers = () => invoke<McpServerInfo[]>("list_mcp_servers");
export const testMcpServer = (name: string) =>
  invoke<string[]>("test_mcp_server", { name });

export const setCostOverride = (
  model: string,
  usdPerMtokInput: number,
  usdPerMtokOutput: number,
) => invoke("set_cost_override", { model, usdPerMtokInput, usdPerMtokOutput });
export const removeCostOverride = (model: string) =>
  invoke("remove_cost_override", { model });

export const listProjectFiles = (query: string) =>
  invoke<string[]>("list_project_files", { query });

export interface PromptPart {
  role: string;
  label: string;
  content: string;
  tokens: number;
}

export interface PromptTool {
  name: string;
  description: string;
  schema: string;
  tokens: number;
}

export interface PromptInspect {
  model: string;
  sent: boolean;
  messages: PromptPart[];
  tools: PromptTool[];
  totalTokens: number;
}

export const inspectPrompt = (sessionId?: string) =>
  invoke<PromptInspect>("inspect_prompt", { sessionId: sessionId ?? null });

export interface UpdateInfo {
  available: boolean;
  current: string;
  latest?: string;
  url?: string;
  releaseNotes?: string;
}

export const checkForUpdate = (force = false) =>
  invoke<UpdateInfo>("check_for_update", { force });

export const openReleaseUrl = (url: string) => invoke("open_release_url", { url });

export const installUpdate = () => invoke("install_update");

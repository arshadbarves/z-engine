// settings (ported in the settings pass): v1 wrappers kept so the Settings
// tabs compile until the Rust config crate exposes its generated types.
import { invoke } from "@tauri-apps/api/core";

export interface PricingInfo {
  usdPerMtokInput: number;
  usdPerMtokOutput: number;
}

export interface McpServerInfo {
  name: string;
  command: string;
  args: string[];
}

export type TaskReportView = "quiet" | "compact" | "detailed";

export interface HarnessConfig {
  model: string;
  maxContextTokens: number;
  maxOutputTokens?: number;
  compactAtPercent?: number;
  baseUrl?: string;
  reviewEnabled?: boolean;
  maxTaskContinuations?: number;
  taskReportView: TaskReportView;
  hasApiKey?: boolean;
  apiKeyHint?: string | null;
  pricing?: PricingInfo | null;
  mcpServers?: McpServerInfo[];
  version?: string;
  projectName?: string;
}

export const getConfig = () => invoke<HarnessConfig>("get_config");

export interface GeneralPatch {
  model?: string | null;
  baseUrl?: string | null;
  maxContextTokens?: number | null;
  review?: boolean | null;
  maxTaskContinuations?: number | null;
  taskReportView?: TaskReportView | null;
}

export const saveGeneral = (p: GeneralPatch) =>
  invoke("save_general", {
    model: p.model ?? null,
    baseUrl: p.baseUrl ?? null,
    maxContextTokens: p.maxContextTokens ?? null,
    review: p.review ?? null,
    maxTaskContinuations: p.maxTaskContinuations ?? null,
    taskReportView: p.taskReportView ?? null,
  });

export const saveApiKey = (key: string | null) => invoke("save_api_key", { key });

export const listPermissionRules = () => invoke<string[]>("list_permission_rules");
export const savePermissionRule = (rule: string) => invoke("save_permission_rule", { rule });
export const removePermissionRule = (rule: string) =>
  invoke("remove_permission_rule", { rule });

export const listMcpServers = () => invoke<McpServerInfo[]>("list_mcp_servers");
export const saveMcpServer = (name: string, command: string, args: string[]) =>
  invoke("save_mcp_server", { name, command, args });
export const removeMcpServer = (name: string) => invoke("remove_mcp_server", { name });
export const testMcpServer = (name: string) => invoke<string[]>("test_mcp_server", { name });

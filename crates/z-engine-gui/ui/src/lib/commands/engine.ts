import { invoke } from "@tauri-apps/api/core";
import type { Command } from "../protocol/Command";
import type { Message } from "../protocol/Message";
import type { SessionSummary } from "../protocol/SessionSummary";

export type SlashCommandSource = "builtin" | "user" | "project" | "mcp";
export type SlashCommandKind = "engine" | "prompt" | "ui";

export interface SlashCommandInfo {
  name: string;
  description: string;
  argumentHint: string | null;
  source: SlashCommandSource;
  kind: SlashCommandKind;
}

export interface AgentCard {
  name: string;
  description: string;
  source: string;
  model: string | null;
  color: string | null;
}

export type ExportFormat = "markdown" | "json";

export type SessionFileKind = "added" | "modified" | "deleted";

export interface SessionChangedFile {
  path: string;
  kind: SessionFileKind;
}

/** Opens (or creates, when `sessionId` is null) a session; the engine then emits its snapshot. */
export const openSession = (projectRoot: string, sessionId: string | null) =>
  invoke<string>("open_session", { projectRoot, sessionId });

export const sendCommand = (sessionId: string, command: Command) =>
  invoke<void>("send_command", { sessionId, command });

export const listSessions = () => invoke<SessionSummary[]>("list_sessions");

export const deleteSession = (sessionId: string) =>
  invoke<void>("delete_session", { sessionId });

export const agentTranscript = (sessionId: string, agentId: string) =>
  invoke<Message[]>("agent_transcript", { sessionId, agentId });

export const exportSession = (sessionId: string, format: ExportFormat) =>
  invoke<string>("export_session", { sessionId, format });

/** Last model request JSON, or null when no request was sent yet. */
export const inspectRequest = (sessionId: string) =>
  invoke<unknown>("inspect_request", { sessionId });

export const listCommands = (projectRoot: string) =>
  invoke<SlashCommandInfo[]>("list_commands", { projectRoot });

export const listFiles = (projectRoot: string, query: string, limit: number) =>
  invoke<string[]>("list_files", { projectRoot, query, limit });

export const listAgents = (projectRoot: string) =>
  invoke<AgentCard[]>("list_agents", { projectRoot });

export const sessionChangedFiles = (sessionId: string) =>
  invoke<SessionChangedFile[]>("session_changed_files", { sessionId });

export const sessionDiffForFile = (sessionId: string, path: string) =>
  invoke<string>("session_diff_for_file", { sessionId, path });

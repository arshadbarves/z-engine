/** Shared frontend types. Runtime, domain, and screens all import from here. */

export type MsgKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "approval"
  | "notice"
  | "command"
  | "error"
  | "status";

export interface Msg {
  id: number;
  kind: MsgKind;
  text: string;
  streaming?: boolean;
  ok?: boolean;
  approvalId?: number;
  canPersist?: boolean;
  suggestedRule?: string | null;
  bashCommand?: string | null;
  /** Unified-diff rich preview for approval cards. */
  detailPreview?: string | null;
  toolName?: string;
  preview?: string;
  summary?: string;
  /** Accumulated stdout/stderr while a bash call runs. */
  output?: string;
  startedAt?: number;
  durationMs?: number;
  thinkingBody?: string;
  collapsed?: boolean;
  /** 0-based index of this user message among turns in the current app run. */
  runTurn?: number | null;
  images?: string[];
}

export interface Toast {
  id: number;
  text: string;
  tone: "info" | "ok" | "warn";
}

export interface Usage {
  promptTokens: number;
  completionTokens: number;
  maxTokens: number;
}

export type SessionActivity = "working" | "approval";

export interface QueuedMessage {
  text: string;
  images: string[];
}

export type EventPayload = { type: string } & Record<string, unknown>;

export interface ReplayToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface ReplayEvent {
  type: string;
  text?: string;
  content?: string | null;
  tool_calls?: ReplayToolCall[];
  tool_call_id?: string;
  model?: string;
  project_root?: string;
  images?: string[];
}

export type Listener = () => void;

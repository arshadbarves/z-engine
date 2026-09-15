/** Shared frontend types. Runtime, domain, and screens all import from here. */
import type { TaskReport } from "./domain/taskReport";

export type { CheckEvidence, EvidenceArtifact, TaskReport, TaskStatus } from "./domain/taskReport";

export type MsgKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "approval"
  | "notice"
  | "command"
  | "error"
  | "task"
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
  taskId?: string;
  taskReport?: TaskReport;
  taskReportError?: string;
  taskFreshnessPending?: boolean;
}

export interface ToastAction {
  label: string;
  onclick?: () => void;
  variant?: "primary" | "secondary";
}

export interface Toast {
  id: number;
  text: string;
  title?: string;
  tag?: string;
  tone: "info" | "ok" | "warn" | "error";
  actions?: ToastAction[];
  onDismiss?: () => void;
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

export type TaskUpdatedEvent = {
  type: "taskUpdated";
  report: TaskReport;
  sessionId?: string;
};

export interface TaskUpdatedReplayEvent {
  type: "task_updated";
  report: TaskReport;
}

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
  /** Journal data is validated before it enters the transcript. */
  report?: unknown;
}

export type Listener = () => void;

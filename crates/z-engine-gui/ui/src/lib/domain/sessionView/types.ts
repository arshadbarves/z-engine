import type { AgentInfo } from "../../protocol/AgentInfo";
import type { ApprovalRequest } from "../../protocol/ApprovalRequest";
import type { CheckRecord } from "../../protocol/CheckRecord";
import type { CheckpointInfo } from "../../protocol/CheckpointInfo";
import type { CompactionMarker } from "../../protocol/CompactionMarker";
import type { ContextBreakdown } from "../../protocol/ContextBreakdown";
import type { Effort } from "../../protocol/Effort";
import type { JobInfo } from "../../protocol/JobInfo";
import type { Message } from "../../protocol/Message";
import type { NoticeLevel } from "../../protocol/NoticeLevel";
import type { PendingPlan } from "../../protocol/PendingPlan";
import type { PendingQuestion } from "../../protocol/PendingQuestion";
import type { PermissionMode } from "../../protocol/PermissionMode";
import type { SessionInfo } from "../../protocol/SessionInfo";
import type { SessionStatus } from "../../protocol/SessionStatus";
import type { TodoItem } from "../../protocol/TodoItem";
import type { ToolStatus } from "../../protocol/ToolStatus";
import type { TurnRecord } from "../../protocol/TurnRecord";
import type { Usage } from "../../protocol/Usage";
import type { VerificationOutcome } from "../../protocol/VerificationOutcome";
import type { JsonValue } from "../../protocol/serde_json/JsonValue";

/** The session's root agent id (`AgentId::main()`). */
export const MAIN_AGENT = "main";

/** Live `toolProgress` output kept per call (the card shows the tail). */
export const PROGRESS_TAIL_CHARS = 8 * 1024;

export type ToolCallStatus = "running" | ToolStatus;

export interface ToolCallView {
  callId: string;
  agentId: string;
  tool: string;
  title: string;
  input: JsonValue;
  status: ToolCallStatus;
  summary: string;
  output: string;
  progress: string;
  durationMs: number | null;
  startedAt: number;
}

export interface StreamingMessage {
  messageId: string;
  agentId: string;
  text: string;
  thinking: string;
  startedAt: number;
}

export interface ActiveTurn {
  turnId: string;
  messageId: string;
  startedAt: number;
}

export interface NoticeView {
  id: number;
  level: NoticeLevel;
  text: string;
  at: number;
}

export interface HookRunView {
  id: number;
  hookEvent: string;
  command: string;
  blocked: boolean;
  message: string | null;
  at: number;
}

/** Ephemeral transcript card anchored after the message that was last when it arrived. */
export interface CommandOutputView {
  id: number;
  name: string;
  markdown: string;
  afterMessageId: string | null;
  at: number;
}

export interface ErrorView {
  id: number;
  message: string;
  afterMessageId: string | null;
  at: number;
}

/** An untrusted project asked for trust (`trustRequired`); cleared by a snapshot or an answer. */
export interface TrustRequestView {
  projectRoot: string;
  /** What stays off until trusted, e.g. `hooks`, `MCP servers`, `checks`. */
  defines: string[];
}

export interface RetryingView {
  attempt: number;
  delayMs: number;
  reason: string;
  at: number;
}

export interface SessionView {
  sessionId: string;
  lastSeq: number;
  info: SessionInfo | null;
  status: SessionStatus;
  /** Main-agent transcript only; subagent messages load on demand. */
  messages: Message[];
  streaming: Record<string, StreamingMessage>;
  tools: Record<string, ToolCallView>;
  approvals: Record<string, ApprovalRequest>;
  questions: Record<string, PendingQuestion>;
  plans: Record<string, PendingPlan>;
  todos: Record<string, TodoItem[]>;
  agents: Record<string, AgentInfo>;
  jobs: Record<string, JobInfo>;
  checks: CheckRecord[];
  turns: TurnRecord[];
  /** User message id -> turn id, for every turn start the GUI has seen. */
  turnStarts: Record<string, string>;
  steeringIds: Record<string, true>;
  activeTurn: ActiveTurn | null;
  checkpoints: CheckpointInfo[];
  compactions: CompactionMarker[];
  usage: Usage;
  agentUsage: Record<string, Usage>;
  costUsd: number;
  contextTokens: number;
  contextLimit: number;
  contextBreakdown: ContextBreakdown | null;
  queue: string[];
  mode: PermissionMode;
  model: string;
  effort: Effort | null;
  title: string | null;
  /** Live badge while the stop boundary runs checks; the turn record keeps the final one. */
  verification: VerificationOutcome | null;
  notices: NoticeView[];
  hooks: HookRunView[];
  outputs: CommandOutputView[];
  errors: ErrorView[];
  lastError: string | null;
  retrying: RetryingView | null;
  trustRequest: TrustRequestView | null;
  nextLocalId: number;
}

export function emptyUsage(): Usage {
  return {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    reasoningTokens: 0,
  };
}

export function emptyView(sessionId: string): SessionView {
  return {
    sessionId,
    lastSeq: -1,
    info: null,
    status: "idle",
    messages: [],
    streaming: {},
    tools: {},
    approvals: {},
    questions: {},
    plans: {},
    todos: {},
    agents: {},
    jobs: {},
    checks: [],
    turns: [],
    turnStarts: {},
    steeringIds: {},
    activeTurn: null,
    checkpoints: [],
    compactions: [],
    usage: emptyUsage(),
    agentUsage: {},
    costUsd: 0,
    contextTokens: 0,
    contextLimit: 0,
    contextBreakdown: null,
    queue: [],
    mode: "default",
    model: "",
    effort: null,
    title: null,
    verification: null,
    notices: [],
    hooks: [],
    outputs: [],
    errors: [],
    lastError: null,
    retrying: null,
    trustRequest: null,
    nextLocalId: 1,
  };
}

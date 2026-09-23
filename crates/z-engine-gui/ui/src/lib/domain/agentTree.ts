import type { AgentInfo } from "../protocol/AgentInfo";
import type { AgentStatus } from "../protocol/AgentStatus";
import type { JobInfo } from "../protocol/JobInfo";
import type { Usage } from "../protocol/Usage";
import { MAIN_AGENT } from "./sessionView/types";
import { sumUsage } from "./usage";

export interface AgentNode {
  info: AgentInfo;
  depth: number;
  childCount: number;
}

const TERMINAL: AgentStatus[] = ["completed", "failed", "cancelled"];

export function isAgentDone(status: AgentStatus): boolean {
  return TERMINAL.includes(status);
}

/** Depth-first agent tree (children under their parent, siblings by start time). */
export function agentTree(agents: Record<string, AgentInfo>): AgentNode[] {
  const all = Object.values(agents).sort((a, b) => a.startedAt - b.startedAt);
  const children = new Map<string, AgentInfo[]>();
  const roots: AgentInfo[] = [];
  for (const agent of all) {
    const parent = agent.parentId;
    if (parent && parent !== agent.agentId && parent in agents) {
      const list = children.get(parent);
      if (list) list.push(agent);
      else children.set(parent, [agent]);
    } else {
      roots.push(agent);
    }
  }
  const out: AgentNode[] = [];
  const seen = new Set<string>();
  const walk = (agent: AgentInfo, depth: number) => {
    if (seen.has(agent.agentId)) return;
    seen.add(agent.agentId);
    const kids = children.get(agent.agentId) ?? [];
    out.push({ info: agent, depth, childCount: kids.length });
    for (const kid of kids) walk(kid, depth + 1);
  };
  for (const root of roots) walk(root, 0);
  return out;
}

export interface WorkCounts {
  runningAgents: number;
  runningJobs: number;
  pendingWorktrees: number;
}

export function workCounts(agents: Record<string, AgentInfo>, jobs: Record<string, JobInfo>): WorkCounts {
  const list = Object.values(agents);
  return {
    runningAgents: list.filter((a) => !isAgentDone(a.status)).length,
    runningJobs: Object.values(jobs).filter((j) => j.status === "running").length,
    pendingWorktrees: list.filter((a) => a.worktree?.state === "pending").length,
  };
}

export interface AgentUsageRow {
  agentId: string;
  label: string;
  usage: Usage;
  costUsd: number | null;
}

/**
 * Main first, then every subagent. The main row's cost is the session cost
 * minus what subagents report, since usage events carry session totals.
 */
export function agentUsageRows(
  agents: Record<string, AgentInfo>,
  agentUsage: Record<string, Usage>,
  sessionCost: number,
): AgentUsageRow[] {
  const subs = agentTree(agents).map(({ info }) => ({
    agentId: info.agentId,
    label: `${info.agentType} · ${info.description}`,
    usage: agentUsage[info.agentId] ?? info.usage,
    costUsd: info.costUsd,
  }));
  const subCost = subs.reduce((n, r) => n + (r.costUsd ?? 0), 0);
  const mainUsage = agentUsage[MAIN_AGENT] ?? sumUsage([]);
  return [
    { agentId: MAIN_AGENT, label: "Main agent", usage: mainUsage, costUsd: Math.max(0, sessionCost - subCost) },
    ...subs,
  ];
}

import type { Msg } from "./events";
import { resolveApproval } from "./events";
import { approveWithRule, deny } from "./commands";

export async function handleApprove(
  m: Msg,
  decision: "once" | "session" | "persist",
): Promise<void> {
  if (m.approvalId == null) return;
  let rule = "";
  let effective = decision;
  if (decision !== "once") {
    if (m.suggestedRule) {
      rule = m.suggestedRule;
    } else {
      effective = "once";
    }
  }
  resolveApproval(m.approvalId, decision);
  await approveWithRule(m.approvalId, effective, rule);
}

export function handleDeny(m: Msg): void {
  if (m.approvalId == null) return;
  resolveApproval(m.approvalId, "deny");
  void deny(m.approvalId);
}

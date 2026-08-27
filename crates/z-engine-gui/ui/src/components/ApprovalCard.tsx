import type { Msg } from "../lib/events";
import { approvalCommand, approvalToolName } from "../lib/approvalPreview";
import { looksLikeDiff } from "../lib/diffParse";
import { DiffView } from "./DiffView";

/** Approval card with four scope buttons; file edits show a colored
 * unified diff instead of a JSON dump. */
export function ApprovalCard({
  m,
  onApprove,
  onDeny,
}: {
  m: Msg;
  onApprove: (decision: "once" | "session" | "persist") => void;
  onDeny: () => void;
}) {
  const tool = approvalToolName(m);
  const command = approvalCommand(m);
  const preview = m.detailPreview ?? null;
  const isDiff = preview != null && looksLikeDiff(preview);

  return (
    <div className="msg approval">
      <div className="approval-kicker">Needs approval</div>
      <div className="approval-title">{tool}</div>
      {isDiff && preview && <DiffView text={preview} />}
      {!isDiff && command && (
        <pre className="approval-cmd">
          <code>{command}</code>
        </pre>
      )}
      <div className="approval-actions">
        <button className="primary" onClick={() => onApprove("once")}>
          Allow once
        </button>
        <button onClick={() => onApprove("session")}>Always · session</button>
        {m.canPersist && <button onClick={() => onApprove("persist")}>Always · persist</button>}
        <button className="deny" onClick={onDeny}>
          Deny
        </button>
        <span className="hint">
          <kbd>y</kbd> once <kbd>s</kbd> session {m.canPersist && (
            <>
              <kbd>p</kbd> persist{" "}
            </>
          )}
          <kbd>n</kbd> deny
        </span>
      </div>
    </div>
  );
}

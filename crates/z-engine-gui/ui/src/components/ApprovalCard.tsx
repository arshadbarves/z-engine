import { ShieldAlert } from "../lib/icons";
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
      <div className="approval-kicker">
        <ShieldAlert size={13} className="approval-kicker-icon" />
        <span>Needs approval</span>
      </div>
      <div className="approval-title">{tool}</div>
      {isDiff && preview && <DiffView text={preview} />}
      {!isDiff && command && (
        <pre className="approval-cmd">
          <code>{command}</code>
        </pre>
      )}
      <div className="approval-actions">
        <button className="primary" onClick={() => onApprove("once")} type="button">
          Allow once
        </button>
        <button onClick={() => onApprove("session")} type="button">
          Always · session
        </button>
        {m.canPersist && (
          <button onClick={() => onApprove("persist")} type="button">
            Always · persist
          </button>
        )}
        <button className="deny" onClick={onDeny} type="button">
          Deny
        </button>
        <span className="hint">
          <kbd>y</kbd> once <kbd>s</kbd> session{" "}
          {m.canPersist && (
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

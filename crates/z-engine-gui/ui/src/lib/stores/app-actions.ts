import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { handleApprove, handleDeny } from "../approvalDispatch";
import { createWorktree, deleteSession, listSessions, submit } from "../commands";
import {
  drainReadyQueues,
  parkCurrentAndReset,
  pushToast,
  sessionStore,
  submitOnSession,
} from "../runtime";
import { hydrateNewSession, hydrateOpenSession } from "../sessionOpen";
import {
  applyFirstUserTitle,
  mergeSessionLists,
  titledSessions,
  ulidFromPath,
} from "../sessionList";
import { refreshCustomCommands } from "../slash";
import type { Msg } from "../types";
import type { SessionEntry } from "../util";
import { sameWorkspacePath, workspaceStore, wsBasename } from "../workspaces";

export type PendingNew = { ulid: string; path: string; projectRoot: string | null } | null;

export async function refreshSessions(
  setList: (fn: (prev: SessionEntry[]) => SessionEntry[]) => void,
) {
  try {
    const list = (await listSessions()) as unknown as SessionEntry[];
    setList((prev) => titledSessions(mergeSessionLists(list, prev)));
  } catch (e) {
    console.error(e);
  }
}

export async function openSession(
  path: string,
  projectRoot: string | null | undefined,
  refresh: () => Promise<void>,
) {
  if (projectRoot) workspaceStore.setActive(projectRoot);
  await hydrateOpenSession(path, projectRoot);
  await refresh();
}

export async function newTask(refresh: () => Promise<void>): Promise<PendingNew> {
  const root = workspaceStore.getSnapshot().active;
  const created = await hydrateNewSession(root);
  void refreshCustomCommands();
  void refresh();
  return created?.path
    ? { ulid: created.ulid, path: created.path, projectRoot: root }
    : null;
}

export async function addWorkspace() {
  try {
    const picked = await openFileDialog({
      directory: true,
      multiple: false,
      title: "Choose a workspace folder",
    });
    if (typeof picked === "string" && picked) {
      await workspaceStore.add(picked);
      pushToast(`Workspace added · ${wsBasename(picked)}`, "ok");
    }
  } catch (e) {
    console.error(e);
    pushToast("Could not add workspace", "warn");
  }
}

export async function removeWorkspace(
  root: string,
  sessionsList: SessionEntry[],
  setList: (fn: (prev: SessionEntry[]) => SessionEntry[]) => void,
  startNew: () => Promise<void>,
  refresh: () => Promise<void>,
) {
  const chats = sessionsList.filter((s) => sameWorkspacePath(s.projectRoot, root));
  const activeId = sessionStore.getSnapshot();
  const deletingActive =
    sameWorkspacePath(workspaceStore.getSnapshot().active, root) ||
    chats.some((s) => s.ulid === activeId);
  setList((prev) => prev.filter((s) => !sameWorkspacePath(s.projectRoot, root)));
  try {
    await workspaceStore.remove(root);
    pushToast(
      chats.length > 0
        ? `Workspace and ${chats.length} chat${chats.length === 1 ? "" : "s"} deleted`
        : "Workspace deleted",
      "info",
    );
  } catch (e) {
    console.error(e);
    pushToast("Could not delete workspace", "warn");
    await refresh();
    return;
  }
  if (deletingActive) {
    const next = workspaceStore.getSnapshot().active;
    if (next) await startNew();
    else parkCurrentAndReset();
  }
  await refresh();
}

export async function delSession(
  path: string,
  setList: (fn: (prev: SessionEntry[]) => SessionEntry[]) => void,
  startNew: () => Promise<void>,
  refresh: () => Promise<void>,
) {
  const id = ulidFromPath(path);
  const wasActive = id === sessionStore.getSnapshot();
  setList((prev) => prev.filter((s) => s.path !== path && s.ulid !== id));
  try {
    await deleteSession(path);
    pushToast("Chat deleted", "info");
  } catch (e) {
    console.error(e);
    pushToast("Delete failed", "warn");
    await refresh();
    return;
  }
  if (wasActive) await startNew();
  await refresh();
}

export async function createWorktreeAndStart(name: string, startNew: () => Promise<void>) {
  try {
    const root = await createWorktree(name);
    await workspaceStore.load();
    workspaceStore.setActive(root);
    pushToast(`Worktree created · ${wsBasename(root)}`, "ok");
    await startNew();
  } catch (e) {
    console.error(e);
    pushToast(String(e).replace("Error: ", ""), "warn");
  }
}

export function flushReadyQueues() {
  const jobs = drainReadyQueues();
  for (const job of jobs) {
    submitOnSession(job.sessionId, job.text, job.images);
    void submit(job.text, job.images, job.sessionId).catch((e) => {
      console.error(e);
      pushToast(String(e).replace("Error: ", ""), "warn");
    });
  }
}

export function applyUserTitle(
  messages: Msg[],
  sessionId: string,
  pendingNew: PendingNew,
  setList: (fn: (prev: SessionEntry[]) => SessionEntry[]) => void,
) {
  setList((prev) =>
    applyFirstUserTitle(
      prev,
      sessionId,
      messages,
      pendingNew?.ulid === sessionId ? pendingNew : null,
    ),
  );
}

export { handleApprove, handleDeny };

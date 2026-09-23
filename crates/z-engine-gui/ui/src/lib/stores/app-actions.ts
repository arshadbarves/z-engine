import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { createWorktree } from "../commands";
import {
  deleteChat,
  errorText,
  newChat,
  openSession,
  pushToast,
  sessionList,
  sessions,
} from "../runtime";
import { resetShell } from "../shellStore";
import { sameWorkspacePath, workspaceStore, wsBasename } from "../workspaces";
import { composer } from "./composer.svelte";

/** Pick a folder and register it as a workspace; returns the new root. */
export async function addWorkspace(): Promise<string | null> {
  try {
    const picked = await openFileDialog({ directory: true, multiple: false, title: "Choose a workspace folder" });
    if (typeof picked !== "string" || !picked) return null;
    await workspaceStore.add(picked);
    pushToast(`Workspace added · ${wsBasename(picked)}`, "ok");
    return workspaceStore.getSnapshot().active;
  } catch (e) {
    console.error(e);
    pushToast("Could not add workspace", "warn");
    return null;
  }
}

/** New chat in the active workspace, asking for a folder when there is none. */
export async function startNewChat(): Promise<void> {
  if (!workspaceStore.getSnapshot().roots.length && !(await addWorkspace())) return;
  if (await newChat()) {
    composer.clear();
    resetShell();
    composer.focus();
  }
}

export async function openChat(sessionId: string, projectRoot: string): Promise<void> {
  if (sessions.activeId === sessionId) return;
  resetShell();
  await openSession(sessionId, projectRoot);
}

export async function removeChat(sessionId: string): Promise<void> {
  await deleteChat(sessionId);
}

export async function removeWorkspace(root: string): Promise<void> {
  const activeRoot = sessions.active?.info?.projectRoot;
  try {
    await workspaceStore.remove(root);
  } catch (e) {
    console.error(e);
    pushToast("Could not remove workspace", "warn");
    return;
  }
  pushToast(`Workspace removed · ${wsBasename(root)}`, "info");
  if (sameWorkspacePath(activeRoot, root)) sessions.activate(null);
  void sessionList.refresh();
}

export async function createWorktreeAndStart(name: string): Promise<void> {
  try {
    const root = await createWorktree(name);
    await workspaceStore.load();
    workspaceStore.setActive(root);
    pushToast(`Worktree created · ${wsBasename(root)}`, "ok");
    await startNewChat();
  } catch (e) {
    console.error(e);
    pushToast(errorText(e), "warn");
  }
}

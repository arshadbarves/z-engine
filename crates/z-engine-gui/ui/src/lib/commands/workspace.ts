import { invoke } from "@tauri-apps/api/core";

export const listWorkspaces = () => invoke<string[]>("list_workspaces");
export const addWorkspace = (path: string) => invoke<string>("add_workspace", { path });
export const removeWorkspace = (path: string) => invoke("remove_workspace", { path });

export interface ChangedFile {
  path: string;
  /** `added` | `modified` | `deleted` */
  status: string;
  added: number;
  deleted: number;
}

export const listChangedFiles = () => invoke<ChangedFile[]>("list_changed_files");
export const diffForFile = (path: string) => invoke<string>("diff_for_file", { path });

export const createWorktree = (name: string) => invoke<string>("create_worktree", { name });

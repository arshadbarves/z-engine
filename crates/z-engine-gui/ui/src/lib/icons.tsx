import type { JSX } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  Alert02Icon,
  ArrowDown01Icon,
  ArrowDown02Icon,
  ArrowLeft01Icon,
  ArrowRight01Icon,
  ArrowRight02Icon,
  ArrowShrink01Icon,
  ArrowUp02Icon,
  Attachment01Icon,
  Brain02Icon,
  Cancel01Icon,
  CheckmarkCircle02Icon,
  Copy01Icon,
  CornerDownLeftIcon,
  Delete02Icon,
  Download01Icon,
  ExternalLinkIcon,
  EyeIcon,
  File01Icon,
  FileAddIcon,
  FileCodeIcon,
  FilePenLineIcon,
  Folder02Icon,
  FolderGitTwoIcon,
  GitBranchIcon,
  GitCompareIcon,
  InformationCircleIcon,
  KeyRoundIcon,
  Loading03Icon,
  MessageSquareIcon,
  OctagonAlertIcon,
  PanelLeftIcon,
  Refresh01Icon,
  Search02Icon,
  ServerIcon,
  Settings03Icon,
  Shield02Icon,
  ShieldAlertIcon,
  SlidersHorizontalIcon,
  SparklesIcon,
  SquareTerminalIcon,
  StopIcon,
  TerminalIcon,
  Tick02Icon,
  Undo02Icon,
  User02Icon,
  WorkflowIcon,
  Wrench02Icon,
} from "@hugeicons/core-free-icons";

export type IconProps = {
  size?: number | string;
  strokeWidth?: number;
  className?: string;
  color?: string;
  fill?: string;
};

export type IconComponent = (props: IconProps) => JSX.Element;

/** Stroke-rounded Hugeicons wrapper matching the modern rounded design language. */
function icon(svg: IconSvgElement): IconComponent {
  function Icon({
    size = 16,
    strokeWidth = 1.5,
    className,
    color = "currentColor",
  }: IconProps) {
    const px = typeof size === "string" ? Number.parseFloat(size) || 16 : size;
    return (
      <HugeiconsIcon
        icon={svg}
        size={px}
        strokeWidth={strokeWidth}
        className={className}
        color={color}
      />
    );
  }
  return Icon;
}

export const Plus = icon(Add01Icon);
export const Settings = icon(Settings03Icon);
export const Search = icon(Search02Icon);
export const PanelLeft = icon(PanelLeftIcon);
export const FolderGit2 = icon(FolderGitTwoIcon);
export const GitCompare = icon(GitCompareIcon);
export const GitBranch = icon(GitBranchIcon);
export const ChevronDown = icon(ArrowDown01Icon);
export const ChevronRight = icon(ArrowRight01Icon);
export const ChevronLeft = icon(ArrowLeft01Icon);
export const ArrowDown = icon(ArrowDown02Icon);
export const ArrowUp = icon(ArrowUp02Icon);
export const ArrowRight = icon(ArrowRight02Icon);
export const FileCode = icon(FileCodeIcon);
export const RefreshCw = icon(Refresh01Icon);
export const X = icon(Cancel01Icon);
export const Sparkles = icon(SparklesIcon);
export const Info = icon(InformationCircleIcon);
export const Server = icon(ServerIcon);
export const Sliders = icon(SlidersHorizontalIcon);
export const Shield = icon(Shield02Icon);
export const ShieldAlert = icon(ShieldAlertIcon);
export const Folder = icon(Folder02Icon);
export const MessageSquare = icon(MessageSquareIcon);
export const Trash2 = icon(Delete02Icon);
export const CheckCircle2 = icon(CheckmarkCircle02Icon);
export const Download = icon(Download01Icon);
export const ExternalLink = icon(ExternalLinkIcon);
export const LoaderCircle = icon(Loading03Icon);
export const Check = icon(Tick02Icon);
export const Copy = icon(Copy01Icon);
export const AlertTriangle = icon(Alert02Icon);
export const AlertOctagon = icon(OctagonAlertIcon);
export const Minimize2 = icon(ArrowShrink01Icon);
export const Eye = icon(EyeIcon);
export const Brain = icon(Brain02Icon);
export const Terminal = icon(TerminalIcon);
export const Paperclip = icon(Attachment01Icon);
export const CornerDownLeft = icon(CornerDownLeftIcon);
export const Square = icon(StopIcon);
export const Undo2 = icon(Undo02Icon);
export const FileText = icon(File01Icon);
export const FilePlus = icon(FileAddIcon);
export const FilePenLine = icon(FilePenLineIcon);
export const SquareTerminal = icon(SquareTerminalIcon);
export const Workflow = icon(WorkflowIcon);
export const Wrench = icon(Wrench02Icon);
export const KeyRound = icon(KeyRoundIcon);
export const User = icon(User02Icon);


/** macOS vs Windows/Linux modifier labels for shortcuts. */
export function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad/.test(navigator.platform) || /Mac OS/.test(navigator.userAgent);
}

export function modLabel(): string {
  return isMacPlatform() ? "⌘" : "Ctrl+";
}

export function applyPlatformClass(): void {
  document.documentElement.classList.toggle("plat-mac", isMacPlatform());
  document.documentElement.classList.toggle("plat-win", /Win/.test(navigator.platform));
}

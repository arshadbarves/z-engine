<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { dismissToast, toastStore, type Toast, type ToastAction } from "$lib/runtime";
  import Icon, { AlertOctagon, AlertTriangle, Check, Info, X } from "$lib/ui/icons";

  const toasts = bindStore(toastStore);

  function iconFor(tone: Toast["tone"]) {
    if (tone === "ok") return Check;
    if (tone === "warn") return AlertTriangle;
    if (tone === "error") return AlertOctagon;
    return Info;
  }

  function handleActionClick(t: Toast, action: ToastAction) {
    if (action.onclick) action.onclick();
    dismissToast(t.id);
  }
</script>

<aside class="toast-host" aria-live="polite" aria-relevant="additions">
  {#each toasts.current as t (t.id)}
    {#if t.actions && t.actions.length > 0}
      <!-- Rich interactive card (strictly for explicit actionable prompts) -->
      <div class={`toast-card is-rich tone-${t.tone}`} role="status">
        <div class="toast-rich-top">
          <span class="toast-glyph" aria-hidden="true">
            <Icon icon={iconFor(t.tone)} size={11} strokeWidth={2.4} />
          </span>
          <div class="toast-rich-header">
            {#if t.title}
              <span class="toast-title">{t.title}</span>
            {/if}
            {#if t.tag}
              <span class="toast-divider" aria-hidden="true">|</span>
              <span class="toast-tag">{t.tag}</span>
            {/if}
          </div>
          <button
            type="button"
            class="toast-close-btn"
            title="Dismiss"
            onclick={() => dismissToast(t.id)}
            aria-label="Dismiss notification"
          >
            <Icon icon={X} size={11} strokeWidth={2.2} />
          </button>
        </div>
        {#if t.text && (!t.title || t.text !== t.title)}
          <div class="toast-rich-desc">{t.text}</div>
        {/if}
        <div class="toast-actions">
          {#each t.actions as action}
            <button
              type="button"
              class={`toast-btn ${action.variant ?? "secondary"}`}
              onclick={() => handleActionClick(t, action)}
            >
              {action.label}
            </button>
          {/each}
        </div>
      </div>
    {:else}
      <!-- Minimalist blur capsule pill HUD -->
      <div class={`toast-capsule tone-${t.tone}`} role="status">
        <span class="toast-glyph" aria-hidden="true">
          <Icon icon={iconFor(t.tone)} size={11} strokeWidth={2.4} />
        </span>
        <div class="toast-content">
          {#if t.title && t.tag}
            <span class="toast-title">{t.title}</span>
            <span class="toast-divider" aria-hidden="true">|</span>
            <span class="toast-tag">{t.tag}</span>
          {:else if t.title && t.text && t.title !== t.text}
            <span class="toast-title">{t.title}:</span>
            <span class="toast-text">{t.text}</span>
          {:else}
            <span class="toast-text">{t.text}</span>
          {/if}
        </div>
        <button
          type="button"
          class="toast-close-btn"
          title="Dismiss"
          onclick={() => dismissToast(t.id)}
          aria-label="Dismiss notification"
        >
          <Icon icon={X} size={11} strokeWidth={2.2} />
        </button>
      </div>
    {/if}
  {/each}
</aside>

<style>
  .toast-host {
    pointer-events: none;
    position: fixed;
    top: 48px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: max-content;
    max-width: min(520px, calc(100vw - 32px));
  }

  .toast-capsule {
    pointer-events: auto;
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 10px 0 7px;
    border-radius: 9999px;
    background: rgba(18, 18, 22, 0.78);
    backdrop-filter: blur(24px) saturate(190%);
    -webkit-backdrop-filter: blur(24px) saturate(190%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.04);
    animation: toast-capsule-in 0.22s cubic-bezier(0.16, 1, 0.3, 1) both;
    box-sizing: border-box;
    max-width: 100%;
    font-size: 12px;
    letter-spacing: -0.01em;
    user-select: none;
  }

  @keyframes toast-capsule-in {
    from {
      opacity: 0;
      transform: translateY(-8px) scale(0.96);
      filter: blur(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
      filter: blur(0);
    }
  }

  /* ── Luminous Micro-Glyph Badges ───────────────────────────────────────── */

  .toast-glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: all 0.15s ease;
  }
  .tone-ok .toast-glyph {
    background: rgba(16, 185, 129, 0.16);
    border: 1px solid rgba(16, 185, 129, 0.3);
    color: #10b981;
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.3);
  }
  .tone-warn .toast-glyph {
    background: rgba(245, 158, 11, 0.16);
    border: 1px solid rgba(245, 158, 11, 0.3);
    color: #f59e0b;
    box-shadow: 0 0 8px rgba(245, 158, 11, 0.3);
  }
  .tone-error .toast-glyph {
    background: rgba(239, 68, 68, 0.16);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
    box-shadow: 0 0 8px rgba(239, 68, 68, 0.3);
  }
  .tone-info .toast-glyph {
    background: rgba(59, 130, 246, 0.16);
    border: 1px solid rgba(59, 130, 246, 0.3);
    color: #60a5fa;
    box-shadow: 0 0 8px rgba(59, 130, 246, 0.3);
  }

  /* ── Content ───────────────────────────────────────────────────────────── */

  .toast-content {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    line-height: 1;
    white-space: nowrap;
  }
  .toast-text {
    color: #f4f4f5;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 380px;
  }
  .toast-title {
    color: #ffffff;
    font-weight: 600;
  }
  .toast-divider {
    color: rgba(255, 255, 255, 0.28);
    font-size: 11px;
    user-select: none;
  }
  .toast-tag {
    color: rgba(255, 255, 255, 0.6);
    font-family: var(--mono);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Dismiss Button ────────────────────────────────────────────────────── */

  .toast-close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.38);
    border-radius: 50%;
    cursor: pointer;
    flex-shrink: 0;
    margin-left: 2px;
    transition: all 0.15s ease;
  }
  .toast-close-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }

  /* ── Rich Card Variant (when explicit actions exist) ───────────────────── */

  .toast-card.is-rich {
    pointer-events: auto;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    width: min(380px, calc(100vw - 32px));
    border-radius: 12px;
    background: rgba(18, 18, 22, 0.84);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.04);
    animation: toast-capsule-in 0.22s cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  .toast-rich-top { display: flex; align-items: center; gap: 8px; }
  .toast-rich-header { display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
  .toast-rich-desc { font-size: 12px; line-height: 1.45; color: rgba(255, 255, 255, 0.72); padding-left: 26px; word-break: break-word; }
  .toast-actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 2px; padding-left: 26px; }
  .toast-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: inherit;
    font-size: 11.5px;
    font-weight: 500;
    letter-spacing: -0.01em;
    height: 25px;
    padding: 0 11px;
    border-radius: 9999px;
    cursor: pointer;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .toast-btn.secondary {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.85);
  }
  .toast-btn.secondary:hover { color: #ffffff; background: rgba(255, 255, 255, 0.14); border-color: rgba(255, 255, 255, 0.2); }
  .toast-btn.primary {
    background: rgba(255, 255, 255, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.25);
    color: #ffffff;
    font-weight: 550;
  }
  .toast-btn.primary:hover { background: rgba(255, 255, 255, 0.3); border-color: rgba(255, 255, 255, 0.4); }
</style>

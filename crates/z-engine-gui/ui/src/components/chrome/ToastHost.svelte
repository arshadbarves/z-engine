<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { toastStore, type Toast } from "$lib/runtime";
  import Icon, { AlertTriangle, Check, Info } from "$lib/ui/icons";

  const toasts = bindStore(toastStore);

  function iconFor(tone: Toast["tone"]) {
    if (tone === "ok") return Check;
    if (tone === "warn") return AlertTriangle;
    return Info;
  }
</script>

<div class="toast-host" aria-live="polite" aria-relevant="additions">
  {#each toasts.current as t (t.id)}
    <div class={`toast-card tone-${t.tone}`} role="status">
      <span class="toast-icon">
        <Icon icon={iconFor(t.tone)} size={13} strokeWidth={2} />
      </span>
      <span class="toast-text">{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toast-host {
    pointer-events: none;
    position: fixed;
    top: 48px;
    left: 50%;
    z-index: 80;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: min(420px, calc(100vw - 48px));
    transform: translateX(-50%);
  }

  .toast-card {
    display: flex;
    align-items: center;
    gap: 10px;
    width: max-content;
    max-width: 100%;
    padding: 8px 14px 8px 10px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 520;
    letter-spacing: -0.012em;
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    animation: toast-in var(--dur-med) var(--ease-out) both;
    box-shadow: 0 10px 32px rgba(0, 0, 0, 0.45);
  }

  .toast-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .toast-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tone-ok {
    color: #d7f5e8;
    background: rgba(20, 36, 30, 0.88);
    border: 1px solid rgba(94, 106, 210, 0.28);
  }
  .tone-ok .toast-icon {
    background: rgba(94, 106, 210, 0.16);
    color: var(--ok);
  }

  .tone-warn {
    color: #f8e8c8;
    background: rgba(40, 30, 16, 0.9);
    border: 1px solid rgba(214, 158, 72, 0.32);
  }
  .tone-warn .toast-icon {
    background: rgba(214, 158, 72, 0.16);
    color: var(--warn);
  }

  .tone-info {
    color: var(--text);
    background: rgba(28, 28, 32, 0.9);
    border: 1px solid var(--border-strong);
  }
  .tone-info .toast-icon {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-2);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(-10px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
</style>

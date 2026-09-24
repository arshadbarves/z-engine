<script lang="ts">
  import { modeMeta } from "$lib/domain/modes";
  import { totalTokens } from "$lib/domain/usage";
  import { sessions } from "$lib/runtime";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";

  type Props = { branch?: string | null; workspaceName?: string | null };
  let { branch = null, workspaceName = null }: Props = $props();

  const view = $derived(sessions.active);
  const busy = $derived((view?.status ?? "idle") !== "idle");
  const tokens = $derived(view ? totalTokens(view.usage) : 0);
  const context = $derived(
    view && view.contextLimit > 0 ? `${fmtTokens(view.contextTokens)} / ${fmtTokens(view.contextLimit)}` : "–",
  );
</script>

<footer id="statusbar" aria-label="Status Bar">
  <div class="status-item">
    <span class={`pulse-dot green${busy ? " purple" : ""}`} aria-hidden="true"></span>
    <span>Engine: <span class="status-val">{view?.model ? shortModel(view.model) : "–"}</span></span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Context:</span>
    <span class="status-val">{context}</span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Tokens:</span>
    <span class="status-val">{fmtTokens(tokens)}</span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Cost:</span>
    <span class="status-val">{fmtCost(view?.costUsd ?? 0)}</span>
  </div>

  <div class="status-item status-trailing">
    <span>Mode:</span>
    <span class="status-val">{modeMeta(view?.mode ?? "default").label}</span>
    {#if branch || workspaceName}
      <div class="status-sep" aria-hidden="true"></div>
      <span>Branch:</span>
      <span class="status-val">{branch || workspaceName}</span>
    {/if}
  </div>
</footer>

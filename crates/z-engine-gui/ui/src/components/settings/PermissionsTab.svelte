<script lang="ts">
  import { listPermissionRules, removePermissionRule, savePermissionRule } from "$lib/commands";

  let rules = $state<string[]>([]);
  let draft = $state("");

  $effect(() => {
    listPermissionRules()
      .then((r) => {
        rules = r;
      })
      .catch(() => {
        rules = [];
      });
  });

  async function add() {
    const rule = draft.trim();
    if (!rule) return;
    await savePermissionRule(rule);
    draft = "";
    rules = await listPermissionRules();
  }

  async function remove(rule: string) {
    await removePermissionRule(rule);
    rules = await listPermissionRules();
  }
</script>

<div class="tab-body">
  <section class="settings-group">
    <h3>Allow rules</h3>
    <p class="form-note">Bash prefixes that skip approval.</p>
    <ul class="rule-list">
      {#each rules as r}
        <li>
          <code>{r}</code>
          <button class="mini" title={`Remove ${r}`} onclick={() => void remove(r)}>✕</button>
        </li>
      {/each}
      {#if rules.length === 0}
        <li class="none">No rules yet.</li>
      {/if}
    </ul>
    <form
      class="inline-form"
      onsubmit={(e) => {
        e.preventDefault();
        void add();
      }}
    >
      <input bind:value={draft} placeholder='e.g. "cargo test*"' spellcheck={false} />
      <button type="submit" disabled={!draft.trim()}>Add rule</button>
    </form>
  </section>
</div>

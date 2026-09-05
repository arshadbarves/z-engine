<script lang="ts">
  import { listPermissionRules, removePermissionRule, savePermissionRule } from "$lib/commands";
  import Icon, { Check, Plus, Shield, Terminal, Trash2 } from "$lib/ui/icons";

  let rules = $state<string[]>([]);
  let draft = $state("");
  let adding = $state(false);

  const PRESETS = [
    { label: "git status", rule: "git status" },
    { label: "git diff", rule: "git diff" },
    { label: "npm test", rule: "npm test" },
    { label: "cargo test", rule: "cargo test*" },
  ];

  $effect(() => {
    let active = true;
    listPermissionRules()
      .then((r) => {
        if (active) rules = r;
      })
      .catch(() => {
        if (active) rules = [];
      });
    return () => {
      active = false;
    };
  });

  async function addRule(ruleToAdd?: string) {
    const rule = (ruleToAdd ?? draft).trim();
    if (!rule || rules.includes(rule)) return;
    adding = true;
    try {
      await savePermissionRule(rule);
      if (!ruleToAdd) draft = "";
      rules = await listPermissionRules();
    } finally {
      adding = false;
    }
  }

  async function remove(rule: string) {
    await removePermissionRule(rule);
    rules = await listPermissionRules();
  }
</script>

<div class="tab-body permissions-tab">
  <!-- Security Overview Card -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Terminal Safety & Approvals</h3>
      <span class="settings-group-sub">
        Control which terminal commands the assistant can run without asking for approval
      </span>
    </div>

    <div class="settings-card permission-status-card">
      <div class="permission-status-icon">
        <Icon icon={Shield} size={20} />
      </div>
      <div class="permission-status-copy">
        <span class="permission-status-title">Terminal Protection is Active</span>
        <p class="permission-status-desc">
          Commands that modify files or execute scripts will pause and request your one-click approval,
          unless they match one of your pre-approved patterns below.
        </p>
      </div>
    </div>
  </section>

  <!-- Pre-approved Rules List -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Pre-approved Patterns ({rules.length})</h3>
      <span class="settings-group-sub">
        Commands starting with these prefixes run immediately without prompting
      </span>
    </div>

    <div class="settings-card">
      {#if rules.length === 0}
        <div class="permission-empty-card">
          <Icon icon={Terminal} size={22} class="permission-empty-icon" />
          <div class="permission-empty-text">
            <strong>No pre-approved commands</strong>
            <p>Every terminal command will require your manual confirmation before executing.</p>
          </div>
        </div>
      {:else}
        <div class="permission-rules-list">
          {#each rules as r}
            <div class="permission-rule-row">
              <div class="permission-rule-left">
                <span class="permission-term-badge">
                  <Icon icon={Terminal} size={12} />
                </span>
                <code class="permission-rule-code">{r}</code>
                <span class="permission-rule-tag">Auto-run</span>
              </div>
              <button
                type="button"
                class="permission-delete-btn"
                title={`Remove rule "${r}"`}
                onclick={() => void remove(r)}
                aria-label={`Remove rule ${r}`}
              >
                <Icon icon={Trash2} size={13} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Inline Add Form -->
      <form
        class="permission-add-form"
        onsubmit={(e) => {
          e.preventDefault();
          void addRule();
        }}
      >
        <div class="permission-input-wrap">
          <input
            bind:value={draft}
            placeholder="e.g. git status, npm test, cargo test*"
            spellcheck={false}
          />
          <button type="submit" class="permission-add-btn" disabled={!draft.trim() || adding}>
            <Icon icon={Plus} size={13} />
            <span>Allow Prefix</span>
          </button>
        </div>
      </form>
    </div>
  </section>

  <!-- Suggested Presets -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Common Safe Commands</h3>
      <span class="settings-group-sub">Click any standard command below to add it instantly</span>
    </div>

    <div class="permission-presets-row">
      {#each PRESETS as p}
        {@const alreadyAdded = rules.includes(p.rule)}
        <button
          type="button"
          class={`preset-chip-btn${alreadyAdded ? " is-added" : ""}`}
          disabled={alreadyAdded || adding}
          onclick={() => void addRule(p.rule)}
          title={alreadyAdded ? "Already allowed" : `Allow "${p.rule}"`}
        >
          {#if alreadyAdded}
            <Icon icon={Check} size={12} class="chip-icon-added" />
          {:else}
            <Icon icon={Plus} size={12} />
          {/if}
          <code>{p.label}</code>
        </button>
      {/each}
    </div>
  </section>
</div>

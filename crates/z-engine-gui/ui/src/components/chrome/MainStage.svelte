<script lang="ts">
  import { lastPromptId } from "$lib/domain/timeline/blocks";
  import { sessions } from "$lib/runtime";
  import { currentStage } from "$lib/stores/stage.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import { createScrollController } from "$lib/ui/scrollController.svelte";
  import { workspaceStore } from "$lib/workspaces";
  import Composer from "../chat/Composer.svelte";
  import Transcript from "../chat/Transcript.svelte";
  import HomeCards from "../home/HomeCards.svelte";
  import HomeHeader from "../home/HomeHeader.svelte";
  import InboxView from "../inbox/InboxView.svelte";
  import JumpLatest from "./JumpLatest.svelte";

  /**
   * The content sheet: the project home, the chat or the inbox. The composer
   * is one instance throughout: centred on the home screen, it glides to the
   * bottom when the first message turns home into a chat.
   */
  const scroller = createScrollController({ bottomThreshold: 24 });
  const workspaces = bindStore(workspaceStore);
  let transcriptEl: HTMLDivElement | undefined = $state();
  let dock: HTMLDivElement | undefined = $state();
  let dockTop: number | null = null;

  const view = $derived(sessions.active);
  const lastPrompt = $derived(lastPromptId(view?.messages));
  const stage = $derived(currentStage());
  const root = $derived(view?.info?.projectRoot ?? workspaces.current.active);
  const composerShown = $derived(stage === "chat" || (stage === "home" && root !== null));

  $effect(() => scroller.bindContainer(transcriptEl));

  $effect(() => {
    userSignals.scrolledBack = scroller.showJump;
  });

  $effect(() => {
    void view;
    scroller.onContentUpdated(sessions.activeId, lastPrompt);
  });

  $effect.pre(() => {
    void stage;
    dockTop = dock ? dock.getBoundingClientRect().top : null;
  });

  $effect(() => {
    void stage;
    const before = dockTop;
    dockTop = null;
    if (!dock || before === null || prefersReducedMotion()) return;
    const shift = before - dock.getBoundingClientRect().top;
    if (Math.abs(shift) < 4) return;
    dock.animate([{ transform: `translateY(${shift}px)` }, { transform: "none" }], {
      duration: 560,
      easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
    });
  });
</script>

<div class="canvas-pane" data-stage={stage}>
  {#if stage === "chat"}
    <div class="transcript-wrap">
      {#if sessions.hydrating}
        <div class="hydrate-shimmer" aria-label="Restoring chat"></div>
      {/if}
      <div class="edge-fade top" aria-hidden="true"></div>
      <div class="transcript" bind:this={transcriptEl}>
        <Transcript />
      </div>
      <div class="edge-fade bottom" aria-hidden="true"></div>
      {#if scroller.showJump}
        <JumpLatest onJump={() => scroller.jumpToLatest()} />
      {/if}
    </div>
  {:else if stage === "home"}
    <div class="home-top">
      <HomeHeader {root} />
    </div>
  {:else}
    <div class="inbox-wrap">
      <InboxView />
    </div>
  {/if}

  {#if composerShown}
    <div class="composer-dock" class:is-hero={stage === "home"} bind:this={dock}>
      <Composer />
    </div>
  {/if}

  {#if stage === "home" && root}
    <div class="home-bottom">
      <HomeCards {root} />
    </div>
  {/if}
</div>

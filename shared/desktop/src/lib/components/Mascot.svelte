<script lang="ts">
  // #181 — one component for every mascot, source and status. Frames come from
  // the macOS render harness (see scripts/export-mascot-sprites.sh); the same
  // component is used at 21 pt (collapsed), 28 pt (session card) and 15 pt
  // (compact row). The frame clock matches MascotTimeline: 20 fps active,
  // 8 fps idle, stopped while the panel is hidden/asleep/reduced-motion.
  import { onMount } from 'svelte';
  import { fpsFor, frameAt } from '../mascot-animation';
  import { spriteFor } from '../mascot-sprites';
  import type { MascotStatus } from '../mascots';

  type Props = {
    source: string | null | undefined;
    status?: MascotStatus;
    size?: number;
    /** Panel-side override (e.g. a collapsed tab): holds the first frame. */
    hidden?: boolean;
  };
  let {
    source,
    status = 'working',
    size = 15,
    hidden = false,
  }: Props = $props();

  // Vite can only bundle the images it can see at build time.
  const assets = import.meta.glob('../assets/mascots/*.png', {
    eager: true,
    query: '?url',
    import: 'default',
  }) as Record<string, string>;

  let sprite = $derived(spriteFor(source, status));
  let visible = $state(true);
  let awake = $state(true);
  let reduceMotion = $state(false);
  let frame = $state(0);
  let elapsed = 0;

  let fps = $derived(fpsFor({ status, visible, awake, hidden, reduceMotion }));
  let url = $derived(assets[`../assets/mascots/${sprite.sheet.file}`] ?? '');

  // One clock per (source, status, gate). A hidden panel schedules no frames
  // at all; a new source/status starts at its first frame.
  $effect(() => {
    const rate = fps;
    const frames = sprite.sheet.frames;
    void sprite.mascot;
    void status;
    elapsed = 0;
    frame = 0;
    if (rate === 0) return;

    let last = performance.now();
    let handle = 0;
    const tick = (now: number) => {
      elapsed += now - last;
      last = now;
      frame = frameAt(elapsed, rate, frames);
      handle = requestAnimationFrame(tick);
    };
    handle = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(handle);
  });

  let element: HTMLElement;

  onMount(() => {
    awake = document.visibilityState === 'visible';
    const onVisibility = () => {
      awake = document.visibilityState === 'visible';
    };
    document.addEventListener('visibilitychange', onVisibility);

    const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onMotion = () => (reduceMotion = motion.matches);
    onMotion();
    motion.addEventListener('change', onMotion);

    // A hidden panel (display:none, offscreen, behind a collapsed tab) burns
    // no frames — the web equivalent of MascotAnimationGate.setPanelVisible.
    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
    });
    observer.observe(element);

    return () => {
      observer.disconnect();
      document.removeEventListener('visibilitychange', onVisibility);
      motion.removeEventListener('change', onMotion);
    };
  });
</script>

<span
  bind:this={element}
  class="mascot"
  role="img"
  aria-label="{source || 'unknown'} mascot, {status}"
  style:width="{size}px"
  style:height="{size}px"
  style:background-image="url({url})"
  style:background-size="{sprite.sheet.frames * size}px {size}px"
  style:background-position="{-frame * size}px 0"
></span>

<style>
  .mascot {
    display: block;
    flex: none;
    background-repeat: no-repeat;
    image-rendering: pixelated;
  }
</style>

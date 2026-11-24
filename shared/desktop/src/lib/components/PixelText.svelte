<script lang="ts">
  import { onMount } from 'svelte';
  import { pixelTextRects, snapPixelSize } from '../pixel';

  type Props = { text: string; size?: number; color?: string; max?: number };
  let { text, size = 1.05, color = 'currentColor', max }: Props = $props();

  let dpr = $state(1);
  onMount(() => {
    const update = () => (dpr = window.devicePixelRatio || 1);
    update();
    // Moving between monitors or changing zoom changes devicePixelRatio.
    let query: MediaQueryList;
    const watch = () => {
      query = matchMedia(`(resolution: ${dpr}dppx)`);
      query.addEventListener('change', onChange, { once: true });
    };
    const onChange = () => {
      update();
      query?.removeEventListener('change', onChange);
      watch();
    };
    watch();
    return () => query?.removeEventListener('change', onChange);
  });

  let grid = $derived(pixelTextRects(text, max));
  let px = $derived(snapPixelSize(size, dpr));
</script>

<svg
  class="px"
  viewBox="0 0 {grid.width} {grid.height}"
  width={(grid.width * px).toFixed(2)}
  height={(grid.height * px).toFixed(2)}
  shape-rendering="crispEdges"
  role="img"
  aria-label={text}
>
  {#each grid.rects as rect (rect.x + ':' + rect.y)}
    <rect x={rect.x} y={rect.y} width="1" height="1" fill={color} />
  {/each}
</svg>

<style>
  svg.px {
    display: inline-block;
    vertical-align: middle;
    flex: none;
  }
</style>

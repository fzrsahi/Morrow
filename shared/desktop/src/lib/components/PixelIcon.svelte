<script lang="ts">
  import { onMount } from 'svelte';
  import { pixelIconRects, snapPixelSize } from '../pixel';

  type Props = { name: string; size?: number; color?: string };
  let { name, size = 1.35, color = 'currentColor' }: Props = $props();

  let dpr = $state(1);
  onMount(() => {
    const update = () => (dpr = window.devicePixelRatio || 1);
    update();
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

  let grid = $derived(pixelIconRects(name));
  let px = $derived(snapPixelSize(size, dpr));
</script>

<svg
  class="pi"
  viewBox="0 0 {grid.width} {grid.height}"
  width={(grid.width * px).toFixed(2)}
  height={(grid.height * px).toFixed(2)}
  shape-rendering="crispEdges"
  aria-hidden="true"
>
  {#each grid.rects as rect (rect.x + ':' + rect.y)}
    <rect x={rect.x} y={rect.y} width="1" height="1" fill={color} />
  {/each}
</svg>

<style>
  svg.pi {
    display: inline-block;
    vertical-align: middle;
    flex: none;
  }
</style>

<script lang="ts">
  // #191 — draw the blocks `morrow_core::markdown` sent. This file has no
  // Markdown parser: headings, lists, tables, quotes and inline spans arrive
  // already structured. Colours and sizes come from the #completion canvas.
  import type { InlineSpan, MarkdownBlock } from '../types';

  let { blocks }: { blocks: MarkdownBlock[] } = $props();

  async function copy(code: string) {
    try {
      await navigator.clipboard.writeText(code);
    } catch {
      // The clipboard is unavailable in a click-through window; the button
      // still does the one thing it can (it never throws into the panel).
    }
  }
</script>

{#snippet spans(items: InlineSpan[])}
  {#each items as span}
    {#if span.kind === 'code'}<code>{span.text}</code>
    {:else if span.kind === 'link'}
      <a href={span.url} target="_blank" rel="noreferrer noopener">{span.text}</a>
    {:else if span.kind === 'strong'}<strong>{@render spans(span.spans ?? [])}</strong>
    {:else if span.kind === 'emphasis'}<em>{@render spans(span.spans ?? [])}</em>
    {:else}{span.text}{/if}
  {/each}
{/snippet}

{#snippet blocksView(items: MarkdownBlock[])}
  {#each items as block}
    {#if block.kind === 'heading'}
      <h4
        style:font-size={`calc(var(--f) + ${block.level === 1 ? 3 : block.level === 2 ? 2 : block.level === 3 ? 1 : 0}px)`}
      >{@render spans(block.spans)}</h4>
    {:else if block.kind === 'paragraph'}
      <p>{@render spans(block.spans)}</p>
    {:else if block.kind === 'list'}
      {#if block.ordered}
        <ol start={block.start}>
          {#each block.items as item}
            <li>
              {#if item.checkbox}<input type="checkbox" checked={item.checkbox === 'checked'} disabled />{/if}
              {@render blocksView(item.blocks)}
            </li>
          {/each}
        </ol>
      {:else}
        <ul>
          {#each block.items as item}
            <li>
              {#if item.checkbox}<input type="checkbox" checked={item.checkbox === 'checked'} disabled />{/if}
              {@render blocksView(item.blocks)}
            </li>
          {/each}
        </ul>
      {/if}
    {:else if block.kind === 'code'}
      <div class="cb">
        {#if block.language}<span class="lang">{block.language}</span>{/if}
        <button class="cp" aria-label="Copy code" title="Copy code" onclick={() => copy(block.code)}>
          <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">
            <rect x="5.5" y="5.5" width="9" height="9" rx="1.5" />
            <path d="M10.5 3.5H3.5a1 1 0 0 0-1 1v7" />
          </svg>
        </button>
        <pre>{block.code}</pre>
      </div>
    {:else if block.kind === 'table'}
      <div class="table-wrap">
        <table>
          <thead><tr>{#each block.header as cell}<th>{@render spans(cell)}</th>{/each}</tr></thead>
          <tbody>
            {#each block.rows as row}
              <tr>{#each row as cell}<td>{@render spans(cell)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if block.kind === 'quote'}
      <blockquote>{@render blocksView(block.blocks)}</blockquote>
    {:else if block.kind === 'thematic_break'}
      <hr />
    {/if}
  {/each}
{/snippet}

<div class="md">{@render blocksView(blocks)}</div>

<style>
  .md {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font: var(--f) / 1.45 ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.85);
    min-width: 0;
  }
  .md :global(h4) {
    margin: 0;
    font-size: calc(var(--f) + 2px);
    color: rgba(255, 255, 255, 0.95);
  }
  .md :global(p) {
    margin: 0;
  }
  .md :global(strong) {
    color: rgba(255, 255, 255, 0.95);
  }
  .md :global(ul),
  .md :global(ol) {
    margin: 0;
    padding-left: 16px;
  }
  .md :global(li) {
    min-width: 0;
  }
  .md :global(li input) {
    margin: 0 4px 0 -14px;
  }
  .md :global(code) {
    color: #f5bd8a;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 3px;
    padding: 0 3px;
  }
  .md :global(a) {
    color: var(--link);
  }
  .cb {
    position: relative;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 0.5px rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 7px;
  }
  .cb pre {
    margin: 0;
    padding-right: 62px;
    font: inherit;
    white-space: pre;
    max-height: calc(var(--f) * 1.45 * 14);
    overflow: auto;
  }
  .cp {
    position: absolute;
    right: 6px;
    top: 6px;
    width: 20px;
    height: 18px;
    border: 0;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.13);
    color: #fff;
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0;
  }
  .cp:hover {
    background: rgba(255, 255, 255, 0.22);
  }
  .lang {
    position: absolute;
    right: 32px;
    top: 8px;
    font-size: 8px;
    color: rgba(255, 255, 255, 0.45);
  }
  .table-wrap {
    width: 100%;
    max-height: calc(var(--f) * 1.45 * 12);
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: calc(var(--f) - 1px);
  }
  th,
  td {
    border: 0.5px solid rgba(255, 255, 255, 0.18);
    padding: 3px 7px;
    text-align: left;
  }
  blockquote {
    margin: 0;
    padding-left: 9px;
    border-left: 2px solid rgba(255, 255, 255, 0.25);
    opacity: 0.72;
  }
  hr {
    width: 100%;
    border: 0;
    border-top: 0.5px solid rgba(255, 255, 255, 0.18);
    margin: 0;
  }
</style>

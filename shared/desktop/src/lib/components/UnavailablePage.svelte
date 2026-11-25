<script lang="ts">
  // #239 AC 3 — a page that has no controls says so *and* says why, on the page.
  // A greyed-out sidebar entry only says "you cannot go there"; the reader is
  // left guessing whether it is a permission, a bug or an unfinished port. The
  // reason is a prop rather than a shared constant so every page argues for
  // itself, and `settings.test.ts` refuses an empty one.
  type Props = {
    title: string;
    reason: string;
    t: (key: string) => string;
  };
  let { title, reason, t }: Props = $props();
</script>

<h2>{title}</h2>
<div class="empty">
  <p class="why">{t('unavailable_platform_notice')}</p>
  <p class="reason">{reason}</p>
</div>

<style>
  .empty {
    display: grid;
    gap: 6px;
    max-width: 54ch;
    padding: 10px 12px;
    border-left: 2px solid var(--amber, #e8a33d);
    background: rgba(255, 255, 255, 0.03);
  }
  .why {
    margin: 0;
    font-weight: 600;
  }
  .reason {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.8;
  }
</style>

<script lang="ts">
  import type { ChromeAction, ChromeSnapshot } from '../lib/bridge';
  let { snapshot, send }: { snapshot: ChromeSnapshot; send: (action: ChromeAction) => void } = $props();

  function size(bytes: number) {
    if (bytes < 1000) return `${bytes} B`;
    const units = ['kB', 'MB', 'GB', 'TB'];
    let value = bytes / 1000;
    let unit = 0;
    while (value >= 1000 && unit < units.length - 1) { value /= 1000; unit += 1; }
    return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
  }

  let terminalCount = $derived(snapshot.downloads.filter((item) => ['finished', 'failed', 'cancelled'].includes(item.status)).length);
</script>

<section class="surface-page downloads-page" aria-labelledby="downloads-heading">
  <div class="page-width downloads-width">
    <header class="page-title-row"><div><h1 id="downloads-heading">Downloads</h1><p class="downloads-caption">Review requested files before saving them.</p></div>
      {#if terminalCount > 0}<button class="quiet-button" type="button" aria-label="Clear completed and stopped downloads" onclick={() => send({ version: 1, action: 'clear_downloads' })}>Clear list</button>{/if}
    </header>
    {#if snapshot.notice}<p class="surface-notice" role="status" aria-live="polite">{snapshot.notice}</p>{/if}
    {#if snapshot.downloads.length}
      <ul class="download-list" aria-label="Downloaded files">
        {#each snapshot.downloads as item (item.id)}
          <li>
            <div class="download-mark" aria-hidden="true"><svg viewBox="0 0 20 20"><path d="M5 2.8h6l4 4v9.7a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 5 16.5z"/><path d="M11 3v4h4M10 9v5m-2-2 2 2 2-2"/></svg></div>
            <div class="download-copy"><strong title={item.name}>{item.name || 'Unnamed download'}</strong>
              {#if item.status === 'awaiting_approval'}<span>Requested by {item.host}</span>
              {:else if item.status === 'cancelled'}<span>Cancelled</span>
              {:else if item.status === 'downloading'}<span class="download-transfer"><span aria-label={`Transferred ${size(item.bytes)}`}>{size(item.bytes)} transferred</span> · Downloading</span>
              {:else}<span>{size(item.bytes)} · {item.status === 'finished' ? 'Complete' : 'Failed'}</span>{/if}
            </div>
            {#if item.status === 'awaiting_approval'}
              <div class="download-consent" role="group" aria-label={`Choose whether to save ${item.name}`}>
                <button class="download-save" type="button" aria-label={`Save ${item.name}`} onclick={() => send({ version: 1, action: 'decide_download', id: item.id, approve: true })}>Save</button>
                <button class="quiet-button" type="button" aria-label={`Cancel ${item.name}`} onclick={() => send({ version: 1, action: 'decide_download', id: item.id, approve: false })}>Cancel</button>
              </div>
            {:else if item.status === 'downloading'}
              <div class="download-progress" role="progressbar" aria-label={`Download progress for ${item.name}`} aria-valuetext="Progress is indeterminate"><span></span></div>
              <button class="quiet-button" type="button" aria-label={`Cancel download ${item.name}`} onclick={() => send({ version: 1, action: 'cancel_download', id: item.id })}>Cancel</button>
            {:else if item.status === 'finished'}
              <div class="download-consent" role="group" aria-label={`Actions for ${item.name}`}>
                {#if item.openable}<button class="quiet-button" type="button" aria-label={`Open ${item.name}`} onclick={() => send({ version: 1, action: 'open_download', id: item.id })}>Open</button>{/if}
                <button class="quiet-button" type="button" aria-label={`Show ${item.name} in folder`} onclick={() => send({ version: 1, action: 'reveal_download', id: item.id })}>Show in folder</button>
              </div>
            {:else if item.retryable}
              <button class="quiet-button" type="button" aria-label={`Retry ${item.name}`} onclick={() => send({ version: 1, action: 'retry_download', id: item.id })}>Retry</button>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <div class="downloads-empty"><span class="downloads-empty-mark" aria-hidden="true">↓</span><h2>No downloads yet</h2><p>Files you save will appear here.</p></div>
    {/if}
  </div>
</section>

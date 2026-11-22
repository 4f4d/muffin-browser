<script lang="ts">
  import { tick } from 'svelte';
  import type { ChromeAction, ChromeSnapshot, SearchEngine, ThemePreference } from '../lib/bridge';

  let { snapshot, send }: { snapshot: ChromeSnapshot; send: (action: ChromeAction) => void } = $props();
  let confirmingClear = $state(false);
  let clearButton: HTMLButtonElement | undefined = $state();

  async function showClearOptions(show: boolean) {
    confirmingClear = show;
    await tick();
    clearButton?.focus();
  }

  const shortcuts = [
    { action: 'Focus address field', keys: '⌘L' },
    { action: 'New tab', keys: '⌘T' },
    { action: 'New private window', keys: '⇧⌘N' },
    { action: 'Close tab', keys: '⌘W' },
    { action: 'Reopen closed tab', keys: '⇧⌘T' },
    { action: 'Search open tabs', keys: '⇧⌘A' },
    { action: 'Switch to tabs 1–8', keys: '⌘1–8' },
    { action: 'Switch to last tab', keys: '⌘9' },
    { action: 'Find on page', keys: '⌘F' },
    { action: 'Open Downloads', keys: '⇧⌘J' }
  ];

  function setTheme(value: ThemePreference) {
    send({ version: 1, action: 'set_theme', value });
  }

  function setSearchEngine(value: SearchEngine) {
    send({ version: 1, action: 'set_search_engine', value });
  }
</script>

<section class="surface-page settings-page" aria-labelledby="settings-heading">
  <div class="page-width settings-width">
    <header class="page-title-row"><h1 id="settings-heading">Settings</h1></header>
    {#if snapshot.notice}<p class="surface-notice" role="status" aria-live="polite">{snapshot.notice}</p>{/if}

    <section class="settings-section" aria-labelledby="appearance-heading">
      <div class="settings-copy"><h2 id="appearance-heading">Appearance</h2><p>Choose how Muffin follows your display.</p></div>
      <label class="settings-control"><span>Theme</span>
        <select aria-label="Theme" value={snapshot.settings.theme} onchange={(event) => setTheme(event.currentTarget.value as ThemePreference)}>
          <option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option>
        </select>
      </label>
    </section>

    <section class="settings-section" aria-labelledby="search-heading">
      <div class="settings-copy"><h2 id="search-heading">Search</h2><p>Used when you enter words in the address field.</p></div>
      <label class="settings-control"><span>Search provider</span>
        <select aria-label="Search provider" value={snapshot.settings.search_engine} onchange={(event) => setSearchEngine(event.currentTarget.value as SearchEngine)}>
          <option value="duckduckgo">DuckDuckGo</option><option value="google">Google</option><option value="bing">Bing</option>
        </select>
      </label>
    </section>

    <section class="settings-section" aria-labelledby="download-folder-heading">
      <div class="settings-copy"><h2 id="download-folder-heading">Download folder</h2><p>Applies to files you approve from now on.</p></div>
      <div class="settings-control">
        <span class="settings-folder-name" title={snapshot.download_folder ?? 'Downloads'}>{snapshot.download_folder ?? 'Downloads'}</span>
        <div class="settings-folder-actions">
          <button class="quiet-button" type="button" onclick={() => send({ version: 1, action: 'choose_download_folder' })}>Choose folder…</button>
          {#if snapshot.download_folder !== null}<button class="quiet-button" type="button" onclick={() => send({ version: 1, action: 'reset_download_folder' })}>Use default</button>{/if}
        </div>
      </div>
    </section>

    <section class="settings-section settings-options" aria-labelledby="startup-heading">
      <div class="settings-copy"><h2 id="startup-heading">Startup and privacy</h2><p>Choose what Muffin restores and clears when you quit.</p></div>
      <fieldset class="settings-checks">
        <legend class="visually-hidden">Startup and privacy preferences</legend>
        <label><span><strong>Restore previous tabs</strong><small>Reopen your tabs the next time Muffin starts.</small></span>
          <input id="restore-session" type="checkbox" checked={snapshot.settings.restore_session} onchange={(event) => send({ version: 1, action: 'set_restore_session', enabled: event.currentTarget.checked })} />
        </label>
        <label><span><strong>Clear history on exit</strong><small>Remove browsing history when you close Muffin.</small></span>
          <input id="clear-history-on-exit" type="checkbox" checked={snapshot.settings.clear_history_on_exit} onchange={(event) => send({ version: 1, action: 'set_clear_history_on_exit', enabled: event.currentTarget.checked })} />
        </label>
      </fieldset>
    </section>
    <section class="settings-section" aria-labelledby="site-data-heading">
      <div class="settings-copy"><h2 id="site-data-heading">Website data</h2><p>Remove cookies, caches and stored website data from regular windows. This can sign you out.</p></div>
      <div class="settings-data-actions">
        {#if confirmingClear}
          <button bind:this={clearButton} class="quiet-button" type="button" onclick={() => showClearOptions(false)}>Cancel</button>
          <button class="quiet-button site-data-clear" type="button" onclick={() => {
            send({ version: 1, action: 'clear_site_data' });
            void showClearOptions(false);
          }}>Clear website data</button>
        {:else}
          <button bind:this={clearButton} class="quiet-button" type="button" onclick={() => showClearOptions(true)}>Clear website data…</button>
        {/if}
      </div>
    </section>
    <section class="settings-section settings-options" aria-labelledby="keyboard-heading">
      <div class="settings-copy"><h2 id="keyboard-heading">Common keyboard shortcuts</h2><p>⌘ Command · ⇧ Shift</p></div>
      <dl class="settings-shortcuts">
        {#each shortcuts as shortcut (shortcut.action)}
          <div><dt>{shortcut.action}</dt><dd><kbd>{shortcut.keys}</kbd></dd></div>
        {/each}
      </dl>
    </section>
  </div>
</section>

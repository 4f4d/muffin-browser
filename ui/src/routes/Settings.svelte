<script lang="ts">
  import type { ChromeAction, ChromeSnapshot, SearchEngine, ThemePreference } from '../lib/bridge';

  let { snapshot, send }: { snapshot: ChromeSnapshot; send: (action: ChromeAction) => void } = $props();

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
    {#if snapshot.notice}<p class="surface-notice" role="status" aria-live="polite">{snapshot.notice}</p>{/if}
  </div>
</section>

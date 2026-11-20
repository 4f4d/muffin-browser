<script lang="ts">
  import type { ChromeAction, ChromeSnapshot, LocalSuggestion } from '../lib/bridge';

  let { snapshot, send }: { snapshot: ChromeSnapshot; send: (action: ChromeAction) => void } = $props();
  let query = $state('');
  let addingShortcut = $state(false);
  let shortcutTitle = $state('');
  let shortcutUrl = $state('');
  let visibleSuggestions = $derived(snapshot.suggestions.slice(0, 6));

  function queryChanged() {
    send({ version: 1, action: 'query_omnibox', value: query });
  }

  function openUrl(url: string) {
    send({ version: 1, action: 'show_surface', value: 'browse' });
    send({ version: 1, action: 'navigate', value: url });
  }

  function openSuggestion(suggestion: LocalSuggestion) {
    openUrl(suggestion.url);
  }

  function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    const value = query.trim();
    if (value) openUrl(value);
  }

  function addShortcut(event: SubmitEvent) {
    event.preventDefault();
    const title = shortcutTitle.trim();
    const url = shortcutUrl.trim();
    if (!title || !url) return;
    send({ version: 1, action: 'add_shortcut', title, url });
    shortcutTitle = '';
    shortcutUrl = '';
    addingShortcut = false;
  }
</script>

<section class="surface-page start-page" aria-labelledby="start-heading">
  <div class="start-inner">
    <div class="brand-line">
      <svg class="brand-mark" viewBox="0 0 36 36" aria-hidden="true"><path d="M8.3 16.5c0-1.9 1.5-3.4 3.4-3.4.7-3 3.4-5.1 6.5-5.1s5.8 2.1 6.5 5.1c1.9 0 3.4 1.5 3.4 3.4 0 .6-.2 1.2-.5 1.7l-2.4 8.7H11.2l-2.4-8.7c-.3-.5-.5-1.1-.5-1.7Z" /><path d="M13 20.5h10M14.7 23.7h6.6" /></svg>
      <h1 id="start-heading">Muffin</h1>
    </div>

    <form class="start-search" role="search" onsubmit={submitSearch}>
      <label class="visually-hidden" for="start-query">Search or enter an address</label>
      <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m12.5 12.5 4 4" /></svg>
      <input id="start-query" bind:value={query} oninput={queryChanged} placeholder="Search or enter an address" autocomplete="off" spellcheck="false" />
      <span class="search-engine-name">{snapshot.settings.search_engine === 'duckduckgo' ? 'DuckDuckGo' : snapshot.settings.search_engine === 'google' ? 'Google' : 'Bing'}</span>
    </form>

    {#if visibleSuggestions.length > 0}
      <section class="start-suggestions" aria-label="Recent and saved pages">
        {#each visibleSuggestions as suggestion (suggestion.url)}
          <button class="suggestion-row" type="button" onclick={() => openSuggestion(suggestion)}>
            <span class="suggestion-title">{suggestion.title}</span>
            <span class="suggestion-url">{suggestion.url}</span>
            {#if suggestion.bookmarked}<span class="saved-label">Saved</span>{/if}
          </button>
        {/each}
      </section>
    {/if}

    <section class="shortcut-section" aria-labelledby="shortcut-heading">
      <div class="surface-heading-row">
        <h2 id="shortcut-heading">Shortcuts</h2>
        {#if snapshot.shortcuts.length > 0}
          <button class="quiet-button" type="button" onclick={() => addingShortcut = !addingShortcut}>{addingShortcut ? 'Cancel' : 'Add shortcut'}</button>
        {/if}
      </div>

      {#if snapshot.shortcuts.length > 0}
        <ul class="shortcut-list">
          {#each snapshot.shortcuts as shortcut (shortcut.id)}
            <li>
              <button class="shortcut-open" type="button" onclick={() => openUrl(shortcut.url)}>
                <span class="shortcut-title">{shortcut.title}</span>
                <span class="shortcut-url">{shortcut.url}</span>
              </button>
              <button class="row-remove" type="button" aria-label={`Remove shortcut ${shortcut.title}`} title="Remove shortcut" onclick={() => send({ version: 1, action: 'remove_shortcut', id: shortcut.id })}>
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
              </button>
            </li>
          {/each}
        </ul>
      {:else if !addingShortcut}
        <div class="empty-line"><span>No shortcuts yet.</span><button class="quiet-button" type="button" onclick={() => addingShortcut = true}>Add shortcut</button></div>
      {/if}

      {#if addingShortcut}
        <form class="inline-form shortcut-form" onsubmit={addShortcut}>
          <label><span>Name</span><input bind:value={shortcutTitle} maxlength="512" required /></label>
          <label><span>Address</span><input bind:value={shortcutUrl} maxlength="4096" required /></label>
          <button class="text-action" type="submit">Save shortcut</button>
        </form>
      {/if}
    </section>
  </div>
</section>

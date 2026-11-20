<script lang="ts">
  import { tick } from 'svelte';
  import type { ChromeAction, ChromeSnapshot, LocalSuggestion } from '../lib/bridge';
  import StartPage from './StartPage.svelte';
  import Library from './Library.svelte';
  import Settings from './Settings.svelte';

  const initial: ChromeSnapshot = {
    version: 1,
    url: '',
    title: 'New tab',
    loading: false,
    can_back: false,
    can_forward: false,
    status: 'home',
    secure: false,
    tabs: [],
    selected_tab: 0,
    can_restore: false,
    surface: 'browse',
    settings: { theme: 'system', search_engine: 'duckduckgo', restore_session: true, clear_history_on_exit: false },
    suggestions: [],
    history: [],
    bookmarks: [],
    shortcuts: [],
    bookmarked: false,
    notice: ''
  };

  let snapshot = $state(window.muffinChrome?.snapshot ?? initial);
  let address = $state('');
  let editing = $state(false);
  let submitted: string | null = null;
  let tabSearchOpen = $state(false);
  let tabSearch = $state('');
  let activeTab = $derived(snapshot.tabs.find((tab) => tab.id === snapshot.selected_tab));
  let visibleTabs = $derived(snapshot.tabs.filter((tab) => tab.title.toLocaleLowerCase().includes(tabSearch.trim().toLocaleLowerCase())));
  let visibleSuggestions = $derived(snapshot.suggestions.slice(0, 5));

  const bridge = window.muffinChrome;

  function send(action: ChromeAction) {
    if (!bridge) return;
    if (action.action === 'navigate' && action.value.length > 4096) return;
    window.dispatchEvent(new CustomEvent('muffin:action', { detail: action }));
  }

  function submitAddress(event: SubmitEvent) {
    event.preventDefault();
    const value = address.trim();
    if (!value) return;
    submitted = value;
    send({ version: 1, action: 'navigate', value });
  }

  function queryAddress() {
    if (editing) send({ version: 1, action: 'query_omnibox', value: address });
  }

  function openSuggestion(suggestion: LocalSuggestion) {
    send({ version: 1, action: 'show_surface', value: 'browse' });
    send({ version: 1, action: 'navigate', value: suggestion.url });
    address = suggestion.url;
    editing = false;
  }

  function applySurfaceLayout(surface: ChromeSnapshot['surface']) {
    const expanded = surface !== 'browse';
    document.documentElement.classList.toggle('surface-expanded', expanded);
    document.body.classList.toggle('surface-expanded', expanded);
    document.getElementById('app')?.classList.toggle('surface-expanded', expanded);
  }

  function displayedAddress(url: string) {
    return url === 'about:blank' ? '' : url;
  }

  function onAddressKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      address = displayedAddress(snapshot.url);
      editing = false;
      submitted = null;
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function onGlobalKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      bridge?.focusAddress();
    }
  }

  async function closeTabSearch() {
    tabSearch = '';
    tabSearchOpen = false;
    await tick();
    const selectedButton = document.querySelector<HTMLButtonElement>(`[data-tab-id="${snapshot.selected_tab}"]`);
    if (selectedButton) {
      selectedButton.focus();
      return;
    }
    const searchButton = document.querySelector<HTMLButtonElement>('[aria-label="Search tabs"]');
    if (searchButton) searchButton.focus();
    else bridge?.focusAddress();
  }

  async function openTabSearch(announce = true) {
    tabSearchOpen = true;
    if (announce) send({ version: 1, action: 'tab_search' });
    await tick();
    const input = document.querySelector<HTMLInputElement>('#tab-search');
    input?.focus();
    input?.select();
  }

  function onTabSearchKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') closeTabSearch();
    if (event.key === 'Enter' && visibleTabs.length > 0) {
      const tabId = visibleTabs[0].id;
      send({ version: 1, action: 'select_tab', id: tabId });
      tabSearch = '';
      tabSearchOpen = false;
      document.querySelector<HTMLButtonElement>(`[data-tab-id="${tabId}"]`)?.focus();
    }
  }

  function onTabKeydown(event: KeyboardEvent, tabId: number) {
    const index = snapshot.tabs.findIndex((tab) => tab.id === tabId);
    if (index < 0) return;
    const moveLeft = (event.altKey || event.metaKey || event.ctrlKey) && event.shiftKey && event.key === 'ArrowLeft';
    const moveRight = (event.altKey || event.metaKey || event.ctrlKey) && event.shiftKey && event.key === 'ArrowRight';
    if (moveLeft || moveRight) {
      event.preventDefault();
      const nextIndex = Math.max(0, Math.min(snapshot.tabs.length - 1, index + (moveLeft ? -1 : 1)));
      if (nextIndex !== index) send({ version: 1, action: 'move_tab', id: tabId, index: nextIndex });
      return;
    }
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.preventDefault();
      const nextIndex = Math.max(0, Math.min(snapshot.tabs.length - 1, index + (event.key === 'ArrowLeft' ? -1 : 1)));
      const nextTab = snapshot.tabs[nextIndex];
      if (nextTab) {
        send({ version: 1, action: 'select_tab', id: nextTab.id });
        document.querySelector<HTMLButtonElement>(`[data-tab-id="${nextTab.id}"]`)?.focus();
      }
    }
  }

  function tabSearchShortcut() {
    void openTabSearch(false);
  }

  function syncSnapshot(event: Event) {
    const detail = (event as CustomEvent<ChromeSnapshot>).detail;
    if (!detail || detail.version !== 1) return;
    applySurfaceLayout(detail.surface);
    document.documentElement.dataset.theme = detail.settings.theme;
    const committedNavigation =
      detail.status === 'ready' && (detail.url !== snapshot.url || snapshot.loading) ||
      detail.status === 'home' && detail.url !== snapshot.url;
    if (committedNavigation && submitted !== null && address.trim() === submitted) {
      editing = false;
      submitted = null;
    }
    snapshot = detail;
    if (!editing) address = displayedAddress(detail.url);
  }

  $effect(() => {
    applySurfaceLayout(snapshot.surface);
    document.documentElement.dataset.theme = snapshot.settings.theme;
    window.addEventListener('muffin:render', syncSnapshot);
    window.addEventListener('keydown', onGlobalKeydown);
    window.addEventListener('muffin:focus-tab-search', tabSearchShortcut);
    return () => {
      window.removeEventListener('muffin:render', syncSnapshot);
      window.removeEventListener('keydown', onGlobalKeydown);
      window.removeEventListener('muffin:focus-tab-search', tabSearchShortcut);
    };
  });
</script>

<main class="chrome" class:expanded={snapshot.surface !== 'browse'} aria-label="Browser controls">
  <div class="toolbar">
    <nav class="history" aria-label="Page history">
      <button class="icon-button" aria-label="Go back" title="Go back" disabled={!snapshot.can_back} onclick={() => send({ version: 1, action: 'back' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M8 4 2.8 9.2 8 14.4M3.2 9.2h8.1a5.1 5.1 0 0 1 0 10.2H8.8" /></svg>
      </button>
      <button class="icon-button" aria-label="Go forward" title="Go forward" disabled={!snapshot.can_forward} onclick={() => send({ version: 1, action: 'forward' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12 4 5.2 5.2-5.2 5.2m4.8-5.2H8.7a5.1 5.1 0 0 0 0 10.2h2.5" /></svg>
      </button>
      <button class="icon-button reload" aria-label={snapshot.loading ? 'Stop loading' : 'Reload page'} title={snapshot.loading ? 'Stop' : 'Reload'} onclick={() => send({ version: 1, action: snapshot.loading ? 'stop' : 'reload' })}>
        {#if snapshot.loading}
          <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5.7" y="5.7" width="8.6" height="8.6" rx="1.8" /></svg>
        {:else}
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M16.3 8a6.4 6.4 0 1 0 .1 3.2M16.2 3.8v4.5h-4.5" /></svg>
        {/if}
      </button>
    </nav>

    <form class="address-shell" role="search" onsubmit={submitAddress}>
      <span class="security-mark" class:secure={snapshot.secure && !editing} aria-hidden="true">
        {#if snapshot.secure && !editing}
          <svg viewBox="0 0 20 20"><rect x="4.6" y="8.6" width="10.8" height="8" rx="2" /><path d="M6.8 8.5V6a3.2 3.2 0 1 1 6.4 0v2.5" /></svg>
        {:else}
          <svg viewBox="0 0 20 20"><circle cx="10" cy="10" r="6.5" /><path d="M3.8 10h12.4M10 3.5a10.2 10.2 0 0 1 0 13M10 3.5a10.2 10.2 0 0 0 0 13" /></svg>
        {/if}
      </span>
      <label class="visually-hidden" for="address">Search or enter address</label>
      <input
        id="address"
        name="address"
        type="text"
        inputmode="url"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        placeholder="Search or enter address"
        bind:value={address}
        onfocus={() => editing = true}
        oninput={queryAddress}
        onblur={() => {
          editing = false;
          submitted = null;
          address = displayedAddress(snapshot.url);
        }}
        onkeydown={onAddressKeydown}
        aria-label="Search or enter address"
      />
      {#if snapshot.status === 'loading'}
        <span class="status-label" aria-live="polite">Loading</span>
      {:else if snapshot.status === 'blocked'}
        <span class="status-label status-error" aria-live="polite">Blocked</span>
      {:else if snapshot.status === 'failed' || snapshot.status === 'terminated'}
        <span class="status-label status-error" aria-live="polite">{snapshot.status === 'terminated' ? 'Stopped' : 'Unavailable'}</span>
      {/if}
      <button class="clear-button" type="button" aria-label="Clear address" title="Clear address" onclick={() => {
        address = '';
        document.getElementById('address')?.focus();
      }}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
      </button>
    </form>

    <button class="home-button" aria-label="Open start page" title="Open start page" onclick={() => send({ version: 1, action: 'show_surface', value: 'start' })}>
      <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m3.5 9 6.5-5.5L16.5 9v7a1.5 1.5 0 0 1-1.5 1.5h-3.2v-5h-3.6v5H5A1.5 1.5 0 0 1 3.5 16z" /></svg>
    </button>
  </div>

  <div class="tabbar" role="group" aria-label="Open tabs and tab actions">
    {#if snapshot.surface === 'browse' && editing && visibleSuggestions.length > 0}
      <div class="browser-suggestions" role="listbox" aria-label="Local suggestions">
        {#each visibleSuggestions as suggestion (suggestion.url)}
          <button class="browser-suggestion" role="option" aria-selected="false" type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => openSuggestion(suggestion)}>
            <span>{suggestion.title}</span><small>{suggestion.url}</small>{#if suggestion.bookmarked}<span class="suggestion-saved">Saved</span>{/if}
          </button>
        {/each}
      </div>
    {:else}
    <div class="tab-list-scroll">
      <div class="tab-list" role="tablist" aria-label="Open tabs">
        {#each visibleTabs as tab (tab.id)}
          <div class="tab-item" role="presentation" class:selected={tab.id === snapshot.selected_tab} class:pinned={tab.pinned} class:discarded={tab.discarded}>
            <button
              class="tab-select"
              role="tab"
              aria-selected={tab.id === snapshot.selected_tab}
              aria-label={`${tab.title || 'New tab'}${tab.pinned ? ', pinned' : ''}${tab.discarded ? ', discarded' : ''}`}
              title={tab.title || 'New tab'}
              tabindex={tab.id === snapshot.selected_tab ? 0 : -1}
              data-tab-id={tab.id}
              onclick={() => send({ version: 1, action: 'select_tab', id: tab.id })}
              onkeydown={(event) => onTabKeydown(event, tab.id)}
            >
              {#if tab.loading}<span class="tab-loading" aria-hidden="true"></span>{:else if tab.pinned}<span class="tab-pin-mark" aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m5 2 6 0-.8 3.2 2.2 2.3v.7H3.6v-.7l2.2-2.3zM8 8.2v5.3" /></svg></span>{:else if tab.discarded}<span class="tab-discard-mark" aria-hidden="true"></span>{/if}
              <span class="tab-title">{tab.title || 'New tab'}</span>
            </button>
            <button
              class="tab-discard"
              aria-label={`Unload ${tab.title || 'tab'}; may lose unsaved in-page state`}
              title={`Unload tab. May lose unsaved in-page state.`}
              disabled={tab.id === snapshot.selected_tab || tab.loading || tab.discarded}
              onclick={() => {
                if (tab.id !== snapshot.selected_tab && !tab.loading && !tab.discarded) {
                  send({ version: 1, action: 'discard_tab', id: tab.id });
                }
              }}
            >
              <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.5v7m-2.5-2.5L8 9.5l2.5-2.5M3 10.5v2A1.5 1.5 0 0 0 4.5 14h7a1.5 1.5 0 0 0 1.5-1.5v-2" /></svg>
            </button>
            <button class="tab-close" aria-label={`Close ${tab.title || 'tab'}`} title="Close tab" onclick={() => send({ version: 1, action: 'close_tab', id: tab.id })}>
              <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
            </button>
          </div>
        {:else}
          {#if tabSearchOpen}<span class="no-tabs" role="status">No matching tabs</span>{/if}
        {/each}
      </div>
    </div>
    {/if}

    <div class="tab-actions" aria-label="Tab actions">
      {#if tabSearchOpen}
        <form class="tab-search-shell" role="search" onsubmit={(event) => event.preventDefault()}>
          <label class="visually-hidden" for="tab-search">Find a tab</label>
          <input id="tab-search" class="tab-search-input" type="search" bind:value={tabSearch} onkeydown={onTabSearchKeydown} placeholder="Find tabs" aria-label="Find tabs" />
          <button class="mini-action search-close" type="button" aria-label="Close tab search" title="Close search" onclick={closeTabSearch}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
          </button>
        </form>
      {:else}
        <button class="mini-action" aria-label="Search tabs" title="Search tabs" onclick={() => void openTabSearch()}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5" /><path d="m12.2 12.2 4.2 4.2" /></svg>
        </button>
      {/if}
      <span class="tab-action-divider" aria-hidden="true"></span>
      <button class="mini-action" aria-label={activeTab?.pinned ? 'Unpin selected tab' : 'Pin selected tab'} title={activeTab?.pinned ? 'Unpin tab' : 'Pin tab'} disabled={!activeTab} onclick={() => activeTab && send({ version: 1, action: 'pin_tab', id: activeTab.id, pinned: !activeTab.pinned })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7 3 6 0-1 4 3 3v1H5v-1l3-3zM10 11v6" /></svg>
      </button>
      <button class="mini-action" aria-label="Duplicate selected tab" title="Duplicate tab" disabled={!activeTab} onclick={() => activeTab && send({ version: 1, action: 'duplicate_tab', id: activeTab.id })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="6" y="6" width="10" height="10" rx="2" /><path d="M13 6V4a1.5 1.5 0 0 0-1.5-1.5H4A1.5 1.5 0 0 0 2.5 4v7.5A1.5 1.5 0 0 0 4 13h2" /></svg>
      </button>
      <button class="mini-action" aria-label="Restore recently closed tab" title="Restore closed tab" disabled={!snapshot.can_restore} onclick={() => send({ version: 1, action: 'reopen_tab' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 5.5V2.8M4 5.5h2.8M4.2 5.7a6.7 6.7 0 1 1-1 5.5" /><path d="M10 6.5v4l2.5 1.5" /></svg>
      </button>
      <span class="tab-action-divider" aria-hidden="true"></span>
      <button class="mini-action" aria-label="Open new window" title="New window" onclick={() => send({ version: 1, action: 'new_window' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="4" width="12" height="11" rx="2" /><path d="M7 16.5h7.5A2.5 2.5 0 0 0 17 14V7.5" /></svg>
      </button>
      <button class="mini-action new-tab-button" aria-label="Open new tab" title="New tab" onclick={() => send({ version: 1, action: 'new_tab' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 4v12M4 10h12" /></svg>
      </button>
      <span class="tab-action-divider" aria-hidden="true"></span>
      <button class="mini-action surface-nav-button" aria-label="Library" title="Library" aria-pressed={snapshot.surface === 'library'} onclick={() => send({ version: 1, action: 'show_surface', value: 'library' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 3.5h10A1.5 1.5 0 0 1 15.5 5v12L9 13.5 2.5 17V5A1.5 1.5 0 0 1 4 3.5ZM5.5 6.5h6M5.5 9h5" /></svg>
      </button>
      <button class="mini-action surface-nav-button" aria-label="Settings" title="Settings" aria-pressed={snapshot.surface === 'settings'} onclick={() => send({ version: 1, action: 'show_surface', value: 'settings' })}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="3" /><path d="m10 2.5.7 1.7 1.8.7 1.7-.6 1.2 1.2-.6 1.7.7 1.8 1.7.7v1.7l-1.7.7-.7 1.8.6 1.7-1.2 1.2-1.7-.6-1.8.7-.7 1.7H8.3l-.7-1.7-1.8-.7-1.7.6-1.2-1.2.6-1.7-.7-1.8-1.7-.7V9.7l1.7-.7.7-1.8-.6-1.7 1.2-1.2 1.7.6 1.8-.7.7-1.7z" /></svg>
      </button>
    </div>
  </div>

  {#if snapshot.surface === 'start'}
    <StartPage {snapshot} {send} />
  {:else if snapshot.surface === 'library'}
    <Library {snapshot} {send} />
  {:else if snapshot.surface === 'settings'}
    <Settings {snapshot} {send} />
  {/if}
</main>

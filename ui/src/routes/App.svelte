<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { ChromeAction, ChromeSnapshot, LocalSuggestion } from '../lib/bridge';
  import StartPage from './StartPage.svelte';
  import Library from './Library.svelte';
  import Settings from './Settings.svelte';
  import Downloads from './Downloads.svelte';
  import ChromeIcon from '../lib/ChromeIcon.svelte';

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
    notice: '',
    downloads: [],
    download_approval_count: 0,
    page_tools: { zoom_percent: 100, find_found: null, fullscreen: false }
  };

  let snapshot = $state(window.muffinChrome?.snapshot ?? initial);
  let address = $state('');
  let editing = $state(false);
  let suggestionsDismissed = $state(false);
  let activeSuggestionUrl = $state<string | null>(null);
  let submitted: string | null = null;
  let tabSearchOpen = $state(false);
  let tabSearch = $state('');
  let moreOpen = $state(false);
  let pageToolsOpen = $state(false);
  let findQuery = $state('');
  let activeTab = $derived(snapshot.tabs.find((tab) => tab.id === snapshot.selected_tab));
  let selectedTabId = $derived(snapshot.selected_tab);
  let visibleTabs = $derived(snapshot.tabs.filter((tab) => tab.title.toLocaleLowerCase().includes(tabSearch.trim().toLocaleLowerCase())));
  let visibleSuggestions = $derived(snapshot.suggestions.slice(0, 5));
  let showingPageTools = $derived(pageToolsOpen && snapshot.surface === 'browse');
  let suggestionsOpen = $derived((snapshot.surface === 'browse' || snapshot.surface === 'start') && !showingPageTools && editing && !suggestionsDismissed && visibleSuggestions.length > 0);
  let activeSuggestionIndex = $derived(visibleSuggestions.findIndex((suggestion) => suggestion.url === activeSuggestionUrl));
  let tabStripVisible = $derived(!moreOpen && !showingPageTools && !suggestionsOpen);

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
    activeSuggestionUrl = null;
    suggestionsDismissed = false;
    if (editing) send({ version: 1, action: 'query_omnibox', value: address });
  }

  function openSuggestion(suggestion: LocalSuggestion) {
    send({ version: 1, action: 'show_surface', value: 'browse' });
    send({ version: 1, action: 'navigate', value: suggestion.url });
    address = suggestion.url;
    editing = false;
    activeSuggestionUrl = null;
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
    if (event.isComposing) return;
    if (suggestionsOpen) {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        suggestionsDismissed = true;
        activeSuggestionUrl = null;
        return;
      }
      if (event.key === 'Enter' && activeSuggestionIndex >= 0) {
        event.preventDefault();
        openSuggestion(visibleSuggestions[activeSuggestionIndex]);
        return;
      }
      if (!event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
        event.preventDefault();
        const last = visibleSuggestions.length - 1;
        const next = activeSuggestionIndex < 0
          ? (event.key === 'ArrowDown' ? 0 : last)
          : Math.max(0, Math.min(last, activeSuggestionIndex + (event.key === 'ArrowDown' ? 1 : -1)));
        activeSuggestionUrl = visibleSuggestions[next].url;
        document.getElementById(`address-suggestion-${next}`)?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
        return;
      }
    }
    if (event.key === 'Escape') {
      address = displayedAddress(snapshot.url);
      editing = false;
      submitted = null;
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function onGlobalKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing) return;
    if (event.key === 'Escape' && moreOpen) {
      event.preventDefault();
      closeMore();
      return;
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      bridge?.focusAddress();
    }
  }

  function toggleMore() {
    pageToolsOpen = false;
    moreOpen = !moreOpen;
    if (moreOpen) tabSearchOpen = false;
  }

  function closeMore() {
    moreOpen = false;
    void tick().then(() => document.querySelector<HTMLButtonElement>('[aria-label="More browser actions"]')?.focus());
  }

  function runMore(action: ChromeAction) {
    send(action);
    closeMore();
  }

  async function openFind() {
    if (snapshot.surface !== 'browse') return;
    moreOpen = false;
    pageToolsOpen = true;
    await tick();
    const input = document.querySelector<HTMLInputElement>('#page-find');
    input?.focus();
    input?.select();
  }

  function closePageTools() {
    pageToolsOpen = false;
    void tick().then(() => document.querySelector<HTMLButtonElement>('[aria-label="Page tools"]')?.focus());
  }

  function onFindKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); closePageTools(); return; }
    if (event.key === 'Enter') {
      event.preventDefault();
      send({ version: 1, action: 'find_page', value: findQuery, backwards: event.shiftKey });
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
    moreOpen = false;
    pageToolsOpen = false;
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
    if (!event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const last = snapshot.tabs.length - 1;
      const nextIndex = event.key === 'Home' ? 0 : event.key === 'End' ? last
        : Math.max(0, Math.min(last, index + (event.key === 'ArrowLeft' ? -1 : 1)));
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

  function revealSelectedTab(id = selectedTabId) {
    const selected = document.querySelector(`[data-tab-id="${id}"]`)?.closest('.tab-item');
    selected?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
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
    if (detail.selected_tab !== snapshot.selected_tab) {
      address = displayedAddress(detail.url);
      editing = document.activeElement?.id === 'address';
      submitted = null;
      activeSuggestionUrl = null;
      suggestionsDismissed = true;
      moreOpen = false;
      pageToolsOpen = false;
    }
    if (detail.selected_tab !== snapshot.selected_tab || detail.url !== snapshot.url) {
      findQuery = '';
    }
    if (detail.surface !== snapshot.surface && detail.surface !== 'browse' && detail.surface !== 'start') {
      tabSearchOpen = false;
      tabSearch = '';
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

  $effect(() => {
    const selectedId = selectedTabId;
    if (!tabStripVisible) return;
    void tick().then(() => {
      if (selectedId !== snapshot.selected_tab || !tabStripVisible) return;
      revealSelectedTab(selectedId);
    });
  });

  onMount(() => {
    window.addEventListener('muffin:focus-find', openFind);
    return () => window.removeEventListener('muffin:focus-find', openFind);
  });
</script>

<main class="chrome" class:expanded={snapshot.surface !== 'browse'} aria-label="Browser controls">
  <div class="toolbar">
    <nav class="history" aria-label="Page history">
      <button class="icon-button" aria-label="Go back" title="Go back" disabled={!snapshot.can_back} onclick={() => send({ version: 1, action: 'back' })}>
        <ChromeIcon name="back" />
      </button>
      <button class="icon-button" aria-label="Go forward" title="Go forward" disabled={!snapshot.can_forward} onclick={() => send({ version: 1, action: 'forward' })}>
        <ChromeIcon name="forward" />
      </button>
      <button class="icon-button reload" aria-label={snapshot.loading ? 'Stop loading' : 'Reload page'} title={snapshot.loading ? 'Stop' : 'Reload'} onclick={() => send({ version: 1, action: snapshot.loading ? 'stop' : 'reload' })}>
        {#if snapshot.loading}
          <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5.7" y="5.7" width="8.6" height="8.6" rx="1.8" /></svg>
        {:else}
          <ChromeIcon name="reload" />
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
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={suggestionsOpen}
        aria-controls={suggestionsOpen ? 'address-suggestions' : undefined}
        aria-activedescendant={suggestionsOpen && activeSuggestionIndex >= 0 ? `address-suggestion-${activeSuggestionIndex}` : undefined}
        aria-keyshortcuts="Meta+L"
        onfocus={() => editing = true}
        oninput={queryAddress}
        onblur={() => {
          editing = false;
          activeSuggestionUrl = null;
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
        activeSuggestionUrl = null;
        suggestionsDismissed = true;
        document.getElementById('address')?.focus();
        send({ version: 1, action: 'query_omnibox', value: '' });
      }}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
      </button>
    </form>

    <div class="toolbar-actions" aria-label="Browser actions">
      <button class="toolbar-action download-toolbar-button" class:approval-badge={snapshot.download_approval_count > 0} type="button" aria-label={snapshot.download_approval_count > 0 ? `Downloads, ${snapshot.download_approval_count} pending approvals` : 'Downloads'} title="Downloads (⇧⌘J)" aria-keyshortcuts="Meta+Shift+J" aria-pressed={snapshot.surface === 'downloads'} onclick={() => send({ version: 1, action: 'show_surface', value: 'downloads' })}>
        <ChromeIcon name="download" />
        {#if snapshot.download_approval_count > 0}<span class="download-count" aria-hidden="true">{snapshot.download_approval_count}</span>{/if}
      </button>
      <button class="toolbar-action more-button" type="button" aria-label="More browser actions" title="More actions" aria-expanded={moreOpen} aria-controls={moreOpen ? 'more-actions' : undefined} onclick={toggleMore}>
        <ChromeIcon name="more" />
      </button>
    </div>
  </div>

  <div class="tabbar" role="group" aria-label="Open tabs and tab actions">
    {#if showingPageTools}
      <div class="page-tools-strip" role="group" aria-label="Page tools">
        <form class="find-shell" role="search" onsubmit={(event) => event.preventDefault()}>
          <label class="visually-hidden" for="page-find">Find on page</label>
          <input id="page-find" type="search" bind:value={findQuery} onkeydown={onFindKeydown} oninput={() => send({ version: 1, action: 'find_page', value: findQuery, backwards: false })} placeholder="Find on page" />
          <span class="find-result" aria-live="polite">{findQuery.trim() ? snapshot.page_tools.find_found === true ? 'Match found' : snapshot.page_tools.find_found === false ? 'No match' : 'Searching…' : ''}</span>
          <button class="mini-action" type="button" aria-label="Previous match" title="Previous match" onclick={() => send({ version: 1, action: 'find_page', value: findQuery, backwards: true })}>↑</button>
          <button class="mini-action" type="button" aria-label="Next match" title="Next match" onclick={() => send({ version: 1, action: 'find_page', value: findQuery, backwards: false })}>↓</button>
        </form>
        <span class="tool-divider" aria-hidden="true"></span>
        <button class="quiet-button" type="button" aria-label="Zoom out" title="Zoom out" onclick={() => send({ version: 1, action: 'zoom_page', value: 'out' })}>−</button>
        <span class="zoom-value" aria-label={`Page zoom ${snapshot.page_tools.zoom_percent} percent`}>{snapshot.page_tools.zoom_percent}%</span>
        <button class="quiet-button" type="button" aria-label="Zoom in" title="Zoom in" onclick={() => send({ version: 1, action: 'zoom_page', value: 'in' })}>+</button>
        <button class="quiet-button" type="button" aria-label="Reset zoom" title="Reset zoom" onclick={() => send({ version: 1, action: 'zoom_page', value: 'reset' })}>Reset</button>
        <button class="quiet-button" type="button" aria-pressed={snapshot.page_tools.fullscreen} onclick={() => send({ version: 1, action: 'toggle_fullscreen' })}>{snapshot.page_tools.fullscreen ? 'Exit full screen' : 'Full screen'}</button>
        <button class="quiet-button" type="button" aria-label="Print page" onclick={() => send({ version: 1, action: 'print_page' })}>Print</button>
        <button class="quiet-button" type="button" aria-label="Save page as PDF" onclick={() => send({ version: 1, action: 'save_pdf' })}>Save PDF</button>
        {#if snapshot.notice}<span class="find-result" role="status">{snapshot.notice}</span>{/if}
        <button class="mini-action" type="button" aria-label="Close page tools" onclick={closePageTools}>×</button>
      </div>
    {:else if suggestionsOpen}
      <div class="browser-suggestions" id="address-suggestions" role="listbox" aria-label="Local suggestions">
        {#each visibleSuggestions as suggestion, index (suggestion.url)}
          <button class="browser-suggestion" id={`address-suggestion-${index}`} role="option" aria-selected={index === activeSuggestionIndex} tabindex="-1" type="button" onpointerdown={(event) => event.preventDefault()} onclick={() => openSuggestion(suggestion)}>
            <span>{suggestion.title}</span><small>{suggestion.url}</small>{#if suggestion.bookmarked}<span class="suggestion-saved">Saved</span>{/if}
          </button>
        {/each}
      </div>
    {:else if moreOpen}
      <div class="more-actions" id="more-actions" role="group" aria-label="More browser actions">
        <button type="button" class="more-menu-action" aria-label="Open start page" onclick={() => runMore({ version: 1, action: 'show_surface', value: 'start' })}><ChromeIcon name="home" /><span>Start</span></button>
        <button type="button" class="more-menu-action" aria-label={activeTab?.pinned ? 'Unpin selected tab' : 'Pin selected tab'} disabled={!activeTab} onclick={() => activeTab && runMore({ version: 1, action: 'pin_tab', id: activeTab.id, pinned: !activeTab.pinned })}><ChromeIcon name={activeTab?.pinned ? 'unpin' : 'pin'} /><span>{activeTab?.pinned ? 'Unpin' : 'Pin'}</span></button>
        <button type="button" class="more-menu-action" aria-label="Duplicate selected tab" disabled={!activeTab} onclick={() => activeTab && runMore({ version: 1, action: 'duplicate_tab', id: activeTab.id })}><ChromeIcon name="duplicate" /><span>Duplicate</span></button>
        <button type="button" class="more-menu-action" aria-label="Restore recently closed tab" disabled={!snapshot.can_restore} onclick={() => runMore({ version: 1, action: 'reopen_tab' })}><ChromeIcon name="restore" /><span>Restore</span></button>
        <span class="more-divider" aria-hidden="true"></span>
        <button type="button" class="more-menu-action" aria-label="Open new window" onclick={() => runMore({ version: 1, action: 'new_window' })}><ChromeIcon name="window" /><span>Window</span></button>
        <button type="button" class="more-menu-action" aria-label="Library" aria-pressed={snapshot.surface === 'library'} onclick={() => runMore({ version: 1, action: 'show_surface', value: 'library' })}><ChromeIcon name="library" /><span>Library</span></button>
        <button type="button" class="more-menu-action" aria-label="Settings" aria-pressed={snapshot.surface === 'settings'} onclick={() => runMore({ version: 1, action: 'show_surface', value: 'settings' })}><ChromeIcon name="settings" /><span>Settings</span></button>
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

    <button class="mini-action new-tab-button" type="button" aria-label="Open new tab" title="New tab (⌘T)" aria-keyshortcuts="Meta+T" onclick={() => send({ version: 1, action: 'new_tab' })}>
      <ChromeIcon name="plus" />
    </button>

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
        <button class="mini-action" aria-label="Search tabs" title="Search tabs (⇧⌘A)" aria-keyshortcuts="Meta+Shift+A" onclick={() => void openTabSearch()}>
          <ChromeIcon name="search" />
        </button>
      {/if}
      <span class="tab-action-divider" aria-hidden="true"></span>
      <button class="mini-action" aria-label="Page tools" title="Page tools (⌘F)" aria-keyshortcuts="Meta+F" aria-pressed={pageToolsOpen} disabled={snapshot.surface !== 'browse'} onclick={() => void openFind()}>
        <ChromeIcon name="page-tools" />
      </button>
    </div>
  </div>

  {#if snapshot.surface === 'start'}
    <StartPage {snapshot} {send} addressEditing={editing} />
  {:else if snapshot.surface === 'library'}
    <Library {snapshot} {send} />
  {:else if snapshot.surface === 'settings'}
    <Settings {snapshot} {send} />
  {:else if snapshot.surface === 'downloads'}
    <Downloads {snapshot} {send} />
  {/if}
</main>

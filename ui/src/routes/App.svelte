<script lang="ts">
  import type { ChromeAction, ChromeSnapshot } from '../lib/bridge';

  const initial: ChromeSnapshot = {
    version: 1,
    url: '',
    title: 'New tab',
    loading: false,
    can_back: false,
    can_forward: false,
    status: 'home',
    secure: false
  };

  let snapshot = $state(initial);
  let address = $state('');
  let editing = $state(false);
  let submitted: string | null = null;

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

  function onAddressKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      address = snapshot.url;
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

  function syncSnapshot(event: Event) {
    const detail = (event as CustomEvent<ChromeSnapshot>).detail;
    if (!detail || detail.version !== 1) return;
    const committedNavigation =
      detail.status === 'ready' && (detail.url !== snapshot.url || snapshot.loading) ||
      detail.status === 'home' && detail.url !== snapshot.url;
    if (committedNavigation && submitted !== null && address.trim() === submitted) {
      editing = false;
      submitted = null;
    }
    snapshot = detail;
    if (!editing) address = detail.url;
  }

  $effect(() => {
    window.addEventListener('muffin:render', syncSnapshot);
    window.addEventListener('keydown', onGlobalKeydown);
    return () => {
      window.removeEventListener('muffin:render', syncSnapshot);
      window.removeEventListener('keydown', onGlobalKeydown);
    };
  });
</script>

<main class="chrome" aria-label="Browser controls">
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
        onblur={() => {
          editing = false;
          submitted = null;
          address = snapshot.url;
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

    <button class="home-button" aria-label="Open start page" title="Open start page" onclick={() => send({ version: 1, action: 'open_home' })}>
      <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m3.5 9 6.5-5.5L16.5 9v7a1.5 1.5 0 0 1-1.5 1.5h-3.2v-5h-3.6v5H5A1.5 1.5 0 0 1 3.5 16z" /></svg>
    </button>
  </div>
</main>

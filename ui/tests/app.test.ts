import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';
import App from '../src/routes/App.svelte';
import { installChromeBridge } from '../src/lib/bridge';
import { makeSnapshot } from './fixtures';

describe('browser chrome controls', () => {
  let component: ReturnType<typeof mount>;
  let bridgeState: ReturnType<typeof installChromeBridge>;

  beforeAll(() => {
    bridgeState = installChromeBridge();
    component = mount(App, { target: document.body });
  });

  beforeEach(() => {
    while (bridgeState.bridge.take() !== null) {}
  });

  afterAll(() => {
    unmount(component);
    delete document.body.dataset.test;
  });

  it('submits typed address as a versioned navigation request', () => {
    const input = document.querySelector<HTMLInputElement>('#address')!;
    input.value = 'example.test/path';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'navigate', value: 'example.test/path' });
  });

  it('renders native state and routes reload versus stop', async () => {
    bridgeState.bridge.render(makeSnapshot({
      url: 'https://example.test',
      title: '<unsafe title>',
      loading: true,
      can_back: true,
      can_forward: false,
      status: 'loading',
      secure: true,
      tabs: [{ id: 20, title: 'Example', pinned: false, loading: true, discarded: false }],
      selected_tab: 20,
      can_restore: false
    }));
    await tick();
    const reload = document.querySelector<HTMLButtonElement>('.reload')!;
    expect(reload.getAttribute('aria-label')).toBe('Stop loading');
    reload.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'stop' });
    expect(document.querySelector('#address')?.textContent).not.toContain('<unsafe title>');
  });

  it('preserves a blocked submission until the address edit is canceled', async () => {
    const oldAddress = 'https://example.test';
    const snapshot = makeSnapshot({
      url: oldAddress,
      title: 'First page',
      loading: false,
      can_back: true,
      can_forward: false,
      secure: true,
      tabs: [{ id: 20, title: 'First page', pinned: false, loading: false, discarded: false }],
      selected_tab: 20,
      can_restore: false
    });
    bridgeState.bridge.render({ ...snapshot, status: 'ready' });
    await tick();
    const input = document.querySelector<HTMLInputElement>('#address')!;
    input.focus();
    input.value = 'file:///unavailable';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'query_omnibox', value: 'file:///unavailable' });
    document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(bridgeState.bridge.take()?.action).toBe('navigate');
    bridgeState.bridge.render({ ...snapshot, status: 'blocked' });
    await tick();
    expect(input.value).toBe('file:///unavailable');
    expect(document.querySelector('.security-mark')?.classList.contains('secure')).toBe(false);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    expect(input.value).toBe(oldAddress);
  });

  it.each(['browse', 'start'] as const)('selects address suggestions by keyboard on %s and Escape preserves the draft', async (surface) => {
    const snapshot = makeSnapshot({
      surface, url: surface === 'start' ? 'about:blank' : 'https://current.test',
      suggestions: [
        { url: 'https://one.test', title: 'First result', bookmarked: false },
        { url: 'https://two.test', title: 'Second result', bookmarked: true }
      ]
    });
    bridgeState.bridge.render(snapshot);
    await tick();
    const input = document.querySelector<HTMLInputElement>('#address')!;
    const press = (key: string) => input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    input.focus();
    input.value = 'result';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    while (bridgeState.bridge.take() !== null) {}
    await tick();
    expect(input.getAttribute('aria-expanded')).toBe('true');
    expect(document.querySelector('.start-suggestions')).toBeNull();
    press('ArrowDown');
    press('ArrowDown');
    await tick();
    const option = document.getElementById(input.getAttribute('aria-activedescendant')!);
    expect(option?.textContent).toContain('Second result');
    expect(option?.getAttribute('aria-selected')).toBe('true');
    const composition = new KeyboardEvent('keydown', { key: 'Enter', isComposing: true, bubbles: true, cancelable: true });
    input.dispatchEvent(composition);
    expect(composition.defaultPrevented).toBe(false);
    expect(bridgeState.bridge.take()).toBeNull();
    press('Escape');
    await tick();
    expect(input.getAttribute('aria-expanded')).toBe('false');
    expect(input.value).toBe('result');
    expect(document.activeElement).toBe(input);

    input.dispatchEvent(new Event('input', { bubbles: true }));
    while (bridgeState.bridge.take() !== null) {}
    await tick();
    press('ArrowUp');
    press('Enter');
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'show_surface', value: 'browse' });
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'navigate', value: 'https://two.test' });
    expect(bridgeState.bridge.take()).toBeNull();
    input.blur();
  });

  it('clears an address draft when a keyboard shortcut switches tabs', async () => {
    const tabs = [
      { id: 20, title: 'Current', pinned: false, loading: false, discarded: false },
      { id: 40, title: 'New Tab', pinned: false, loading: false, discarded: false }
    ];
    bridgeState.bridge.render(makeSnapshot({ tabs, selected_tab: 20, url: 'https://current.test' }));
    await tick();
    const input = document.querySelector<HTMLInputElement>('#address')!;
    input.focus();
    input.value = 'unfinished search';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    while (bridgeState.bridge.take() !== null) {}
    bridgeState.bridge.render(makeSnapshot({ tabs, selected_tab: 40, url: 'about:blank', surface: 'start' }));
    await tick();
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe('');
    input.value = 'next search';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'query_omnibox', value: 'next search' });
    input.blur();
  });

  it('routes tab lifecycle actions through the typed queue', async () => {
    const tabs = [
      { id: 10, title: 'Mail', pinned: true, loading: false, discarded: false },
      { id: 20, title: 'Reading list', pinned: false, loading: false, discarded: false },
      { id: 30, title: 'Docs home', pinned: false, loading: false, discarded: true }
    ];
    bridgeState.bridge.render(makeSnapshot({
      url: 'https://reading.test',
      title: 'Reading list',
      loading: false,
      can_back: false,
      can_forward: false,
      status: 'ready',
      secure: true,
      tabs,
      selected_tab: 20,
      can_restore: true
    }));
    await tick();

    const newTab = document.querySelector<HTMLButtonElement>('.new-tab-button')!;
    expect(newTab.previousElementSibling?.classList.contains('tab-list-scroll')).toBe(true);
    expect(newTab.getAttribute('aria-keyshortcuts')).toBe('Meta+T');
    newTab.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'new_tab' });
    const openMore = async () => {
      document.querySelector<HTMLButtonElement>('[aria-label="More browser actions"]')!.click();
      await tick();
    };
    await openMore();
    document.querySelector<HTMLButtonElement>('[aria-label="Open new window"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'new_window' });
    await openMore();
    document.querySelector<HTMLButtonElement>('[aria-label="Restore recently closed tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'reopen_tab' });
    await openMore();
    document.querySelector<HTMLButtonElement>('[aria-label="Pin selected tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'pin_tab', id: 20, pinned: true });
    await openMore();
    document.querySelector<HTMLButtonElement>('[aria-label="Duplicate selected tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'duplicate_tab', id: 20 });
    await tick();
    document.querySelector<HTMLButtonElement>('[aria-label="Unload Mail; may lose unsaved in-page state"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'discard_tab', id: 10 });
    const activeTabUnload = document.querySelector<HTMLButtonElement>('[aria-label="Unload Reading list; may lose unsaved in-page state"]')!;
    expect(activeTabUnload.disabled).toBe(true);
    activeTabUnload.click();
    expect(bridgeState.bridge.take()).toBeNull();
    document.querySelector<HTMLButtonElement>('[aria-label="Close Reading list"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'close_tab', id: 20 });
    document.querySelector<HTMLButtonElement>('[data-tab-id="10"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'select_tab', id: 10 });

    document.querySelector<HTMLButtonElement>('[data-tab-id="20"]')!.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'End', bubbles: true, cancelable: true })
    );
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'select_tab', id: 30 });
  });

  it('keeps More actions contained, dismisses on Escape, and restores focus', async () => {
    bridgeState.bridge.render(makeSnapshot({
      tabs: [{ id: 20, title: 'Reading', pinned: false, loading: false, discarded: false }],
      selected_tab: 20
    }));
    await tick();
    const more = document.querySelector<HTMLButtonElement>('[aria-label="More browser actions"]')!;
    more.click();
    await tick();
    expect(more.getAttribute('aria-expanded')).toBe('true');
    expect(document.querySelector('#more-actions [aria-label="Library"]')).not.toBeNull();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    expect(more.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(more);
  });

  it('reveals the selected tab without moving keyboard focus', async () => {
    const scroll = vi.fn();
    const previous = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollIntoView');
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', { configurable: true, value: scroll });
    try {
      const address = document.querySelector<HTMLInputElement>('#address')!;
      address.focus();
      const tabs = Array.from({ length: 9 }, (_, index) => ({ id: 20 + index, title: `Tab ${index + 1}`, pinned: false, loading: false, discarded: false }));
      bridgeState.bridge.render(makeSnapshot({ tabs, selected_tab: 28 }));
      await tick();
      await tick();
      expect(scroll).toHaveBeenCalledWith({ block: 'nearest', inline: 'nearest' });
      expect(document.activeElement).toBe(address);
    } finally {
      if (previous) Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', previous);
      else delete (HTMLElement.prototype as { scrollIntoView?: unknown }).scrollIntoView;
    }
  });

  it('supports keyboard tab reorder and inline tab search', async () => {
    bridgeState.bridge.render(makeSnapshot({
      url: '',
      title: 'Reading list',
      loading: false,
      can_back: false,
      can_forward: false,
      status: 'home',
      secure: false,
      tabs: [
        { id: 10, title: 'Mail', pinned: false, loading: false, discarded: false },
        { id: 20, title: 'Reading list', pinned: false, loading: false, discarded: false },
        { id: 30, title: 'Docs home', pinned: false, loading: false, discarded: false }
      ],
      selected_tab: 20,
      can_restore: false
    }));
    await tick();

    document.querySelector<HTMLButtonElement>('[data-tab-id="20"]')!.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', altKey: true, shiftKey: true, bubbles: true })
    );
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'move_tab', id: 20, index: 2 });

    document.querySelector<HTMLButtonElement>('[data-tab-id="20"]')!.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true })
    );
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'select_tab', id: 10 });

    document.querySelector<HTMLButtonElement>('[aria-label="Search tabs"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'tab_search' });
    await tick();
    const search = document.querySelector<HTMLInputElement>('#tab-search')!;
    search.value = 'docs';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    expect(document.querySelectorAll('[role="tab"]')).toHaveLength(1);
    search.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    await tick();
    expect(document.querySelector('#tab-search')).toBeNull();
    expect(document.activeElement?.getAttribute('data-tab-id')).toBe('20');

    bridgeState.bridge.focusTabSearch();
    await tick();
    await tick();
    expect(document.activeElement?.id).toBe('tab-search');
    const searchAgain = document.querySelector<HTMLInputElement>('#tab-search')!;
    searchAgain.value = 'docs';
    searchAgain.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    searchAgain.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })
    );
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'select_tab', id: 30 });
    await tick();
    expect(document.querySelector('#tab-search')).toBeNull();
  });

  it('falls back to search when a selected tab disappears during search close', async () => {
    await openTabSearchFromNative();
    bridgeState.bridge.render(makeSnapshot({
      url: '',
      title: 'Closed',
      loading: false,
      can_back: false,
      can_forward: false,
      status: 'home',
      secure: false,
      tabs: [],
      selected_tab: 0,
      can_restore: false
    }));
    await tick();
    document.querySelector<HTMLInputElement>('#tab-search')!.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
    );
    await tick();
    await tick();
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Search tabs');
  });

  it('renders the start page and routes local search and shortcut actions', async () => {
    bridgeState.bridge.render(makeSnapshot({
      surface: 'start',
      suggestions: [{ url: 'https://saved.test', title: 'Saved page', bookmarked: true }],
      shortcuts: [{ id: 4, url: 'https://docs.test', title: 'Docs' }]
    }));
    await tick();
    expect(document.body.classList.contains('surface-expanded')).toBe(true);
    expect(document.querySelector('#start-heading')?.textContent).toBe('Muffin');
    const input = document.querySelector<HTMLInputElement>('#start-query')!;
    input.value = 'local search';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'query_omnibox', value: 'local search' });
    document.querySelector<HTMLButtonElement>('.suggestion-row')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'show_surface', value: 'browse' });
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'navigate', value: 'https://saved.test' });
    bridgeState.bridge.render(makeSnapshot({ surface: 'start', shortcuts: [{ id: 4, url: 'https://docs.test', title: 'Docs' }] }));
    await tick();
    document.querySelector<HTMLButtonElement>('[aria-label="Remove shortcut Docs"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'remove_shortcut', id: 4 });
  });

  it('filters and opens library entries and organizes bookmarks into folders', async () => {
    bridgeState.bridge.render(makeSnapshot({
      surface: 'library',
      history: [{ id: 11, url: 'https://news.test', title: 'News', visited_at_ms: 1_700_000_000_000 }],
      bookmarks: [
        { id: 21, parent_id: null, url: null, title: 'Reading' },
        { id: 22, parent_id: null, url: 'https://saved.test', title: 'Saved' }
      ]
    }));
    await tick();
    expect(document.querySelector('.chrome')?.classList.contains('expanded')).toBe(true);
    document.querySelector<HTMLInputElement>('.library-search input')!.value = 'news';
    document.querySelector<HTMLInputElement>('.library-search input')!.dispatchEvent(new Event('input', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'query_library', value: 'news' });
    document.querySelector<HTMLButtonElement>('.history-list .library-open')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'show_surface', value: 'browse' });
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'navigate', value: 'https://news.test' });

    const librarySearch = document.querySelector<HTMLInputElement>('.library-search input')!;
    librarySearch.value = '';
    librarySearch.dispatchEvent(new Event('input', { bubbles: true }));
    bridgeState.bridge.take();
    const bookmarksView = document.querySelector<HTMLButtonElement>('.library-nav [aria-pressed="false"]')!;
    bookmarksView.click();
    await tick();
    expect(bookmarksView.getAttribute('aria-pressed')).toBe('true');
    document.querySelector<HTMLSelectElement>('[aria-label="Move Saved to folder"]')!.value = '21';
    document.querySelector<HTMLSelectElement>('[aria-label="Move Saved to folder"]')!.dispatchEvent(new Event('change', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'move_bookmark', id: 22, parent_id: 21 });
    [...document.querySelectorAll<HTMLButtonElement>('.library-actions button')].find((button) => button.textContent?.includes('New folder'))!.click();
    await tick();
    const folderName = document.querySelector<HTMLInputElement>('.folder-form input')!;
    folderName.value = 'Research';
    folderName.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector<HTMLFormElement>('.folder-form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'create_folder', title: 'Research', parent_id: null });
  });

  it('applies theme preferences and emits explicit settings changes', async () => {
    bridgeState.bridge.render(makeSnapshot({ surface: 'settings' }));
    await tick();
    expect(document.documentElement.dataset.theme).toBe('system');
    const theme = document.querySelector<HTMLSelectElement>('[aria-label="Theme"]')!;
    theme.value = 'dark';
    theme.dispatchEvent(new Event('change', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'set_theme', value: 'dark' });
    const clearOnExit = document.querySelector<HTMLInputElement>('#clear-history-on-exit')!;
    clearOnExit.checked = true;
    clearOnExit.dispatchEvent(new Event('change', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'set_clear_history_on_exit', enabled: true });
    bridgeState.bridge.render(makeSnapshot({ surface: 'browse', settings: { theme: 'light', search_engine: 'duckduckgo', restore_session: true, clear_history_on_exit: false } }));
    await tick();
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.body.classList.contains('surface-expanded')).toBe(false);
  });

  it('shows only the chosen folder name and routes folder changes without a path', async () => {
    bridgeState.bridge.render(makeSnapshot({ surface: 'settings', download_folder: 'Project files' }));
    await tick();
    expect(document.querySelector('.settings-folder-name')?.textContent).toBe('Project files');
    const buttons = document.querySelectorAll<HTMLButtonElement>('.settings-folder-actions button');
    buttons[0].click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'choose_download_folder' });
    buttons[1].click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'reset_download_folder' });
    bridgeState.bridge.render(makeSnapshot({ surface: 'settings' }));
    await tick();
    expect(document.querySelector('.settings-folder-name')?.textContent).toBe('Downloads');
    expect(document.querySelectorAll('.settings-folder-actions button')).toHaveLength(1);
  });

  it('separates private browsing controls from the regular profile', async () => {
    document.querySelector<HTMLButtonElement>('[aria-label="More browser actions"]')!.click();
    await tick();
    document.querySelector<HTMLButtonElement>('[aria-label="Open new private window"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'new_private_window' });
    bridgeState.bridge.render(makeSnapshot({ surface: 'start', private_window: true }));
    await tick();
    expect(document.querySelector('.private-indicator')?.textContent).toBe('Private');
    expect(document.querySelector('#start-heading')?.textContent).toBe('Private browsing');
    expect(document.querySelector('.shortcut-section')).toBeNull();
    document.querySelector<HTMLButtonElement>('[aria-label="More browser actions"]')!.click();
    await tick();
    expect(document.querySelector('#more-actions [aria-label="Library"]')).toBeNull();
    expect(document.querySelector('#more-actions [aria-label="Settings"]')).toBeNull();
  });

  it('sends a clear request only after confirming in Settings', async () => {
    bridgeState.bridge.render(makeSnapshot({ surface: 'settings' }));
    await tick();
    const buttons = () => [...document.querySelectorAll<HTMLButtonElement>('.settings-data-actions button')];
    buttons()[0].click();
    await tick();
    expect(bridgeState.bridge.take()).toBeNull();
    expect(buttons()[0].textContent).toBe('Cancel');
    buttons()[0].click();
    await tick();
    expect(bridgeState.bridge.take()).toBeNull();
    buttons()[0].click();
    await tick();
    buttons()[1].click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'clear_site_data' });
  });

  it('keeps consent text inert and routes approval, cancellation, and reveal actions', async () => {
    bridgeState.bridge.render(makeSnapshot({
      surface: 'downloads',
      download_approval_count: 2,
      downloads: [
        { id: 90, name: '<img src=x onerror=alert(1)>', host: '<script>evil.test</script>', status: 'awaiting_approval', bytes: 0, retryable: false, openable: false },
        { id: 94, name: 'Later file', host: 'download.example', status: 'awaiting_approval', bytes: 0, retryable: false, openable: false },
        { id: 91, name: 'Annual report.pdf', host: 'reports.example', status: 'finished', bytes: 1250000, retryable: false, openable: true },
        { id: 92, name: 'Large archive.zip', host: 'files.example', status: 'downloading', bytes: 2048, retryable: false, openable: false },
        { id: 93, name: 'Cancelled item', host: 'files.example', status: 'cancelled', bytes: 0, retryable: false, openable: false }
      ]
    }));
    await tick();
    expect(document.querySelector('#downloads-heading')?.textContent).toBe('Downloads');
    expect(document.querySelector('.download-copy strong')?.textContent).toBe('<img src=x onerror=alert(1)>');
    expect(document.querySelector('img')).toBeNull();
    expect(document.querySelector('script')).toBeNull();
    expect(document.querySelectorAll('.download-list li')).toHaveLength(5);
    expect(document.querySelector('.download-copy span')?.textContent).toContain('<script>evil.test</script>');
    expect(document.querySelector('.download-copy span')?.textContent).not.toContain('0 B');
    expect(document.querySelector('.download-list li:nth-child(5) .download-copy')?.textContent).toContain('Cancelled');
    expect(document.querySelector<HTMLButtonElement>('.approval-badge')?.textContent).toContain('2');
    document.querySelector<HTMLButtonElement>('.approval-badge')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'show_surface', value: 'downloads' });
    document.querySelector<HTMLButtonElement>('[aria-label="Save <img src=x onerror=alert(1)>"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'decide_download', id: 90, approve: true });
    document.querySelector<HTMLButtonElement>('[aria-label="Cancel Later file"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'decide_download', id: 94, approve: false });
    document.querySelector<HTMLButtonElement>('[aria-label="Show Annual report.pdf in folder"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'reveal_download', id: 91 });
    document.querySelector<HTMLButtonElement>('[aria-label="Open Annual report.pdf"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'open_download', id: 91 });
  });

  it('shows live transferred bytes without a false percentage and clears only terminal rows', async () => {
    bridgeState.bridge.render(makeSnapshot({
      surface: 'downloads',
      downloads: [
        { id: 110, name: 'Waiting.zip', host: 'files.example', status: 'awaiting_approval', bytes: 0, retryable: false, openable: false },
        { id: 111, name: 'Active.zip', host: 'files.example', status: 'downloading', bytes: 2048, retryable: false, openable: false },
        { id: 112, name: 'Installer.pkg', host: 'files.example', status: 'finished', bytes: 4096, retryable: false, openable: false },
        { id: 113, name: 'Broken.bin', host: 'files.example', status: 'failed', bytes: 10, retryable: true, openable: false },
        { id: 114, name: 'Stopped.bin', host: 'files.example', status: 'cancelled', bytes: 10, retryable: false, openable: false }
      ]
    }));
    await tick();
    expect(document.querySelector('[aria-label="Transferred 2.0 kB"]')).not.toBeNull();
    expect(document.querySelector('.download-progress[aria-valuenow]')).toBeNull();
    document.querySelector<HTMLButtonElement>('[aria-label="Retry Broken.bin"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'retry_download', id: 113 });
    expect(document.querySelector('[aria-label="Retry Stopped.bin"]')).toBeNull();
    expect(document.querySelector('[aria-label="Open Installer.pkg"]')).toBeNull();
    expect(document.querySelector('[aria-label="Show Installer.pkg in folder"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[aria-label="Cancel download Active.zip"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'cancel_download', id: 111 });
    document.querySelector<HTMLButtonElement>('[aria-label="Clear completed and stopped downloads"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'clear_downloads' });

    bridgeState.bridge.render(makeSnapshot({
      surface: 'downloads',
      downloads: [
        { id: 110, name: 'Waiting.zip', host: 'files.example', status: 'awaiting_approval', bytes: 0, retryable: false, openable: false },
        { id: 111, name: 'Active.zip', host: 'files.example', status: 'downloading', bytes: 0, retryable: false, openable: false }
      ]
    }));
    await tick();
    expect(document.querySelector('[aria-label="Clear completed and stopped downloads"]')).toBeNull();
  });

  it('handles native Find focus, match navigation, page tools, and Escape', async () => {
    bridgeState.bridge.render(makeSnapshot({ surface: 'browse', page_tools: { zoom_percent: 125, find_found: null, fullscreen: false, media_suspended: false } }));
    await tick();
    bridgeState.bridge.focusFind();
    await tick();
    await tick();
    const find = document.querySelector<HTMLInputElement>('#page-find')!;
    expect(document.activeElement).toBe(find);
    expect(document.querySelector('.find-result')?.textContent).toBe('');
    find.value = 'muffin';
    find.dispatchEvent(new Event('input', { bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'find_page', value: 'muffin', backwards: false });
    find.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', shiftKey: true, bubbles: true }));
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'find_page', value: 'muffin', backwards: true });
    await tick();
    expect(document.querySelector('.find-result')?.textContent).toBe('Searching…');
    bridgeState.bridge.render(makeSnapshot({ surface: 'browse', page_tools: { zoom_percent: 125, find_found: true, fullscreen: false, media_suspended: false } }));
    await tick();
    expect(document.querySelector('.find-result')?.textContent).toBe('Match found');
    document.querySelector<HTMLButtonElement>('[aria-label="Zoom in"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'zoom_page', value: 'in' });
    document.querySelector<HTMLButtonElement>('[aria-label="Reset zoom"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'zoom_page', value: 'reset' });
    document.querySelector<HTMLButtonElement>('[aria-label="Print page"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'print_page' });
    document.querySelector<HTMLButtonElement>('[aria-label="Save page as PDF"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'save_pdf' });
    const mediaButton = () => [...document.querySelectorAll<HTMLButtonElement>('.page-tools-strip button')]
      .find((button) => button.textContent?.includes('all media'))!;
    expect(mediaButton().textContent).toBe('Pause all media');
    mediaButton().click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'set_media_suspended', enabled: true });
    bridgeState.bridge.render(makeSnapshot({ surface: 'browse', page_tools: {
      zoom_percent: 125, find_found: true, fullscreen: false, media_suspended: true,
    } }));
    await tick();
    expect(mediaButton().textContent).toBe('Resume all media');
    mediaButton().click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'set_media_suspended', enabled: false });
    find.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    await tick();
    expect(document.querySelector('#page-find')).toBeNull();
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Page tools');
  });
});

async function openTabSearchFromNative() {
  window.muffinChrome?.focusTabSearch();
  await tick();
  await tick();
}

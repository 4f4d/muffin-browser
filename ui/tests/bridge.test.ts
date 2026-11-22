import { describe, expect, it, vi } from 'vitest';
import { createChromeBridge, isAction, type ChromeSnapshot } from '../src/lib/bridge';

describe('chrome IPC bridge', () => {
  it('returns queued actions in order and bounds the queue', () => {
    const state = createChromeBridge();
    for (let index = 0; index < 16; index += 1) {
      expect(state.enqueue({ version: 1, action: 'back' })).toBe(true);
    }
    expect(state.enqueue({ version: 1, action: 'forward' })).toBe(false);
    expect(state.bridge.take()).toEqual({ version: 1, action: 'back' });
    expect(state.enqueue({ version: 1, action: 'forward' })).toBe(true);
    for (let index = 0; index < 15; index += 1) state.bridge.take();
    expect(state.bridge.take()).toEqual({ version: 1, action: 'forward' });
    expect(state.bridge.take()).toBeNull();
  });

  it('rejects malformed or oversized actions', () => {
    expect(isAction({ version: 2, action: 'back' })).toBe(false);
    expect(isAction({ version: 1, action: 'execute' })).toBe(false);
    expect(isAction({ version: 1, action: 'navigate', value: 'x'.repeat(4097) })).toBe(false);
    expect(isAction({ version: 1, action: 'navigate', value: 'https://example.test' })).toBe(true);
  });

  it('accepts a valid snapshot, ignores malformed updates, and calls focus', () => {
    const focus = vi.fn();
    const focusSearch = vi.fn();
    const state = createChromeBridge(focus, focusSearch);
    const snapshot: ChromeSnapshot = {
      version: 1,
      url: 'https://example.test',
      title: 'Example',
      loading: false,
      can_back: true,
      can_forward: false,
      status: 'ready',
      secure: true,
      tabs: [{ id: 3, title: 'Example', pinned: false, loading: false, discarded: false }],
      selected_tab: 3,
      can_restore: false, private_window: false,
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
      download_folder: null,
      page_tools: { zoom_percent: 100, find_found: null, fullscreen: false, media_suspended: false }
    };
    state.bridge.render(snapshot);
    expect(state.bridge.snapshot).toEqual(snapshot);
    state.bridge.render({ ...snapshot, status: 'unknown' } as unknown as ChromeSnapshot);
    expect(state.bridge.snapshot).toEqual(snapshot);
    state.bridge.render({
      ...snapshot,
      tabs: [
        { id: 3, title: 'Example', pinned: false, loading: false, discarded: false },
        { id: 3, title: 'Duplicate ID', pinned: false, loading: false, discarded: false }
      ]
    });
    expect(state.bridge.snapshot).toEqual(snapshot);
    state.bridge.focusAddress();
    expect(focus).toHaveBeenCalledOnce();
    state.bridge.focusTabSearch();
    expect(focusSearch).toHaveBeenCalledOnce();
  });

  it('validates each tab action variant and snapshot identity', () => {
    expect(isAction({ version: 1, action: 'new_tab' })).toBe(true);
    expect(isAction({ version: 1, action: 'new_window' })).toBe(true);
    expect(isAction({ version: 1, action: 'reopen_tab' })).toBe(true);
    expect(isAction({ version: 1, action: 'tab_search' })).toBe(true);
    expect(isAction({ version: 1, action: 'select_tab', id: 42 })).toBe(true);
    expect(isAction({ version: 1, action: 'close_tab', id: 42 })).toBe(true);
    expect(isAction({ version: 1, action: 'duplicate_tab', id: 42 })).toBe(true);
    expect(isAction({ version: 1, action: 'discard_tab', id: 42 })).toBe(true);
    expect(isAction({ version: 1, action: 'pin_tab', id: 42, pinned: true })).toBe(true);
    expect(isAction({ version: 1, action: 'move_tab', id: 42, index: 2 })).toBe(true);
    expect(isAction({ version: 1, action: 'move_tab', id: 42, index: 256 })).toBe(false);
    expect(isAction({ version: 1, action: 'close_tab', id: -1 })).toBe(false);
    expect(isAction({ version: 1, action: 'pin_tab', id: 42, pinned: 'yes' })).toBe(false);
  });

  it('validates M3 action variants and argument ranges', () => {
    expect(isAction({ version: 1, action: 'show_surface', value: 'library' })).toBe(true);
    expect(isAction({ version: 1, action: 'show_surface', value: 'private' })).toBe(false);
    expect(isAction({ version: 1, action: 'set_theme', value: 'dark' })).toBe(true);
    expect(isAction({ version: 1, action: 'set_theme', value: 'auto' })).toBe(false);
    expect(isAction({ version: 1, action: 'clear_history', value: 'week' })).toBe(true);
    expect(isAction({ version: 1, action: 'clear_history', value: 'month' })).toBe(false);
    expect(isAction({ version: 1, action: 'set_clear_history_on_exit', enabled: true })).toBe(true);
    expect(isAction({ version: 1, action: 'create_folder', title: 'Reference', parent_id: null })).toBe(true);
    expect(isAction({ version: 1, action: 'move_bookmark', id: 5, parent_id: 12 })).toBe(true);
    expect(isAction({ version: 1, action: 'move_bookmark', id: 5, parent_id: -1 })).toBe(false);
    expect(isAction({ version: 1, action: 'add_shortcut', title: 'Docs', url: 'https://example.test' })).toBe(true);
    expect(isAction({ version: 1, action: 'move_bookmark', id: 5, parent_id: null, extra: true })).toBe(false);
    expect(isAction({ version: 1, action: 'find_page', value: 'muffin', backwards: false })).toBe(true);
    expect(isAction({ version: 1, action: 'find_page', value: 'x', backwards: 'no' })).toBe(false);
    expect(isAction({ version: 1, action: 'zoom_page', value: 'reset' })).toBe(true);
    expect(isAction({ version: 1, action: 'toggle_fullscreen' })).toBe(true);
    expect(isAction({ version: 1, action: 'print_page' })).toBe(true);
    expect(isAction({ version: 1, action: 'reveal_download', id: 9 })).toBe(true);
    expect(isAction({ version: 1, action: 'reveal_download', id: 0 })).toBe(false);
    expect(isAction({ version: 1, action: 'open_download', id: 9 })).toBe(true);
    expect(isAction({ version: 1, action: 'open_download', id: 9, path: '/tmp/a' })).toBe(false);
    expect(isAction({ version: 1, action: 'retry_download', id: 9 })).toBe(true);
    expect(isAction({ version: 1, action: 'retry_download', id: 9, url: 'https://example.test/' })).toBe(false);
    expect(isAction({ version: 1, action: 'cancel_download', id: 42 })).toBe(true);
    expect(isAction({ version: 1, action: 'cancel_download', id: -1 })).toBe(false);
    expect(isAction({ version: 1, action: 'cancel_download', id: 42, path: '/tmp/a' })).toBe(false);
    expect(isAction({ version: 1, action: 'clear_downloads' })).toBe(true);
    expect(isAction({ version: 1, action: 'clear_site_data' })).toBe(true);
    expect(isAction({ version: 1, action: 'clear_site_data', origin: 'https://example.test/' })).toBe(false);
    expect(isAction({ version: 1, action: 'choose_download_folder' })).toBe(true);
    expect(isAction({ version: 1, action: 'reset_download_folder' })).toBe(true);
    expect(isAction({ version: 1, action: 'choose_download_folder', path: '/tmp/x' })).toBe(false);
    expect(isAction({ version: 1, action: 'clear_downloads', id: 42 })).toBe(false);
    expect(isAction({ version: 1, action: 'decide_download', id: 9, approve: true })).toBe(true);
    expect(isAction({ version: 1, action: 'decide_download', id: 9, approve: false })).toBe(true);
    expect(isAction({ version: 1, action: 'decide_download', id: 9, approve: 1 })).toBe(false);
    expect(isAction({ version: 1, action: 'decide_download', id: 9, approve: true, path: '/tmp/x' })).toBe(false);
  });

  it('rejects malformed M3 snapshot data and freezes accepted records', () => {
    const state = createChromeBridge();
    const snapshot: ChromeSnapshot = {
      version: 1, url: '', title: 'New tab', loading: false, can_back: false, can_forward: false,
      status: 'home', secure: false, tabs: [], selected_tab: 0, can_restore: false, private_window: false, surface: 'downloads',
      settings: { theme: 'system', search_engine: 'duckduckgo', restore_session: true, clear_history_on_exit: false },
      suggestions: [{ url: 'https://saved.test', title: 'Saved', bookmarked: true }],
      history: [{ id: 1, url: 'https://visited.test', title: 'Visited', visited_at_ms: 1_700_000_000_000 }],
      bookmarks: [{ id: 2, parent_id: null, url: 'https://saved.test', title: 'Saved' }],
      shortcuts: [{ id: 3, url: 'https://docs.test', title: 'Docs' }], bookmarked: true, notice: '',
      downloads: [{ id: 9, name: 'report.pdf', host: 'example.test', status: 'finished', bytes: 1250, retryable: false, openable: false }],
      download_approval_count: 1,
      download_folder: null,
      page_tools: { zoom_percent: 125, find_found: true, fullscreen: false, media_suspended: false }
    };
    state.bridge.render(snapshot);
    expect(state.bridge.snapshot).toEqual(snapshot);
    expect(Object.isFrozen(state.bridge.snapshot.settings)).toBe(true);
    expect(Object.isFrozen(state.bridge.snapshot.history[0])).toBe(true);

    state.bridge.render({ ...snapshot, surface: 'private' } as unknown as ChromeSnapshot);
    expect(state.bridge.snapshot.surface).toBe('downloads');
    state.bridge.render({ ...snapshot, settings: { ...snapshot.settings, search_engine: 'unknown' } } as unknown as ChromeSnapshot);
    expect(state.bridge.snapshot.settings.search_engine).toBe('duckduckgo');
    state.bridge.render({ ...snapshot, downloads: [{ id: 9, name: 'bad', host: 'example.test', status: 'unknown', bytes: 0, retryable: false, openable: false }] } as unknown as ChromeSnapshot);
    expect(state.bridge.snapshot.downloads[0].status).toBe('finished');
    state.bridge.render({ ...snapshot, downloads: [{ id: 9, name: 'bad', host: 'x'.repeat(256), status: 'finished', bytes: 1, retryable: false, openable: false }] });
    expect(state.bridge.snapshot.downloads[0].host).toBe('example.test');
    state.bridge.render({ ...snapshot, download_approval_count: 17 });
    expect(state.bridge.snapshot.download_approval_count).toBe(1);
    state.bridge.render({ ...snapshot, bookmarks: [{ id: 2, parent_id: null, url: 'https://saved.test', title: 'Bad', extra: true }] } as unknown as ChromeSnapshot);
    expect(state.bridge.snapshot.bookmarks[0].url).toBe('https://saved.test');
  });
});

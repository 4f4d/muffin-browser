import type { ChromeSnapshot } from '../src/lib/bridge';

export function makeSnapshot(overrides: Partial<ChromeSnapshot> = {}): ChromeSnapshot {
  // Keep default download data empty: native sends rows only on Downloads but the pending count persists on every surface.
  // Consent tests use hostile markup-like names/hosts to verify remote metadata remains inert text.
  return {
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
    settings: {
      theme: 'system',
      search_engine: 'duckduckgo',
      restore_session: true,
      clear_history_on_exit: false
    },
    suggestions: [],
    history: [],
    bookmarks: [],
    shortcuts: [],
    bookmarked: false,
    notice: '',
    downloads: [],
    download_approval_count: 0,
    download_folder: null,
    page_tools: { zoom_percent: 100, find_found: null, fullscreen: false },
    ...overrides
  };
}

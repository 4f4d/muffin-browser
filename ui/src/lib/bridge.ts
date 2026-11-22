export type ChromeAction =
  | { version: 1; action: 'navigate'; value: string }
  | {
      version: 1;
      action: 'back' | 'forward' | 'reload' | 'stop' | 'focus_page' | 'open_home';
    }
  | { version: 1; action: 'new_tab' | 'new_window' | 'reopen_tab' | 'tab_search' }
  | { version: 1; action: 'select_tab' | 'close_tab' | 'duplicate_tab' | 'discard_tab'; id: number }
  | { version: 1; action: 'pin_tab'; id: number; pinned: boolean }
  | { version: 1; action: 'move_tab'; id: number; index: number }
  | { version: 1; action: 'show_surface'; value: BrowserSurface }
  | { version: 1; action: 'query_omnibox' | 'query_library' | 'set_theme' | 'set_search_engine' | 'clear_history'; value: string }
  | { version: 1; action: 'set_restore_session' | 'set_clear_history_on_exit'; enabled: boolean }
  | { version: 1; action: 'bookmark_current' | 'import_bookmarks' | 'export_bookmarks' }
  | { version: 1; action: 'remove_visit' | 'delete_bookmark' | 'remove_shortcut'; id: number }
  | { version: 1; action: 'create_folder'; title: string; parent_id: number | null }
  | { version: 1; action: 'move_bookmark'; id: number; parent_id: number | null }
  | { version: 1; action: 'add_shortcut'; title: string; url: string }
  | { version: 1; action: 'find_page'; value: string; backwards: boolean }
  | { version: 1; action: 'zoom_page'; value: 'in' | 'out' | 'reset' }
  | { version: 1; action: 'toggle_fullscreen' | 'print_page' | 'save_pdf' }
  | { version: 1; action: 'reveal_download' | 'cancel_download'; id: number }
  | { version: 1; action: 'clear_downloads' | 'choose_download_folder' | 'reset_download_folder' }
  | { version: 1; action: 'decide_download'; id: number; approve: boolean };

export type TabSnapshot = {
  id: number;
  title: string;
  pinned: boolean;
  loading: boolean;
  discarded: boolean;
};

export type BrowserSurface = 'browse' | 'start' | 'library' | 'settings' | 'downloads';
export type DownloadEntry = { id: number; name: string; host: string; status: 'awaiting_approval' | 'downloading' | 'finished' | 'failed' | 'cancelled'; bytes: number };
export type PageToolsState = { zoom_percent: number; find_found: boolean | null; fullscreen: boolean };
export type ThemePreference = 'system' | 'light' | 'dark';
export type SearchEngine = 'duckduckgo' | 'google' | 'bing';
export type LocalSuggestion = { url: string; title: string; bookmarked: boolean };
export type HistoryEntry = { id: number; url: string; title: string; visited_at_ms: number };
export type BookmarkEntry = { id: number; parent_id: number | null; url: string | null; title: string };
export type ShortcutEntry = { id: number; url: string; title: string };
export type BrowserSettings = {
  theme: ThemePreference;
  search_engine: SearchEngine;
  restore_session: boolean;
  clear_history_on_exit: boolean;
};

export type ChromeSnapshot = {
  version: 1;
  url: string;
  title: string;
  loading: boolean;
  can_back: boolean;
  can_forward: boolean;
  status: 'home' | 'loading' | 'ready' | 'blocked' | 'failed' | 'terminated';
  secure: boolean;
  tabs: TabSnapshot[];
  selected_tab: number;
  can_restore: boolean;
  surface: BrowserSurface;
  settings: BrowserSettings;
  suggestions: LocalSuggestion[];
  history: HistoryEntry[];
  bookmarks: BookmarkEntry[];
  shortcuts: ShortcutEntry[];
  bookmarked: boolean;
  notice: string;
  downloads: DownloadEntry[];
  download_approval_count: number;
  download_folder: string | null;
  page_tools: PageToolsState;
};

export type ChromeBridge = {
  take(): ChromeAction | null;
  render(snapshot: ChromeSnapshot): void;
  focusAddress(): void;
  focusTabSearch(): void;
  focusFind(): void;
  readonly snapshot: ChromeSnapshot;
};

const MAX_QUEUE_LENGTH = 16;
const MAX_URL_LENGTH = 4096;
const MAX_TITLE_LENGTH = 512;
const STATUSES = new Set<ChromeSnapshot['status']>([
  'home',
  'loading',
  'ready',
  'blocked',
  'failed',
  'terminated'
]);

const initialSnapshot: ChromeSnapshot = {
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
  page_tools: { zoom_percent: 100, find_found: null, fullscreen: false }
};

export function createChromeBridge(
  focusAddress: () => void = () => undefined,
  focusTabSearch: () => void = () => undefined,
  focusFind: () => void = () => undefined
) {
  const actions: ChromeAction[] = [];
  let currentSnapshot = initialSnapshot;

  const bridge: ChromeBridge = {
    take() {
      return actions.shift() ?? null;
    },
    render(snapshot) {
      if (!isSnapshot(snapshot)) return;
      currentSnapshot = Object.freeze({
        ...snapshot,
        settings: Object.freeze({ ...snapshot.settings }),
        tabs: freezeRows(snapshot.tabs),
        suggestions: freezeRows(snapshot.suggestions),
        history: freezeRows(snapshot.history),
        bookmarks: freezeRows(snapshot.bookmarks),
        shortcuts: freezeRows(snapshot.shortcuts),
        downloads: freezeRows(snapshot.downloads),
        page_tools: Object.freeze({ ...snapshot.page_tools })
      });
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('muffin:render', { detail: currentSnapshot }));
      }
    },
    focusAddress,
    focusTabSearch,
    focusFind,
    get snapshot() {
      return currentSnapshot;
    }
  };

  return {
    bridge,
    enqueue(action: ChromeAction) {
      if (actions.length >= MAX_QUEUE_LENGTH || !isAction(action)) return false;
      actions.push(Object.freeze({ ...action }));
      return true;
    }
  };
}

export function isAction(value: unknown): value is ChromeAction {
  if (!isObject(value) || value.version !== 1 || typeof value.action !== 'string') return false;
  if (value.action === 'navigate') {
    return hasExactKeys(value, ['version', 'action', 'value'])
      && typeof value.value === 'string'
      && value.value.length > 0
      && value.value.length <= MAX_URL_LENGTH;
  }
  if (hasExactKeys(value, ['version', 'action'])
    && ['back', 'forward', 'reload', 'stop', 'focus_page', 'open_home', 'new_tab', 'new_window', 'reopen_tab', 'tab_search'].includes(value.action)) {
    return true;
  }
  if (hasExactKeys(value, ['version', 'action', 'id'])
    && ['select_tab', 'close_tab', 'duplicate_tab', 'discard_tab'].includes(value.action)) {
    return validId(value.id);
  }
  if (value.action === 'pin_tab' && hasExactKeys(value, ['version', 'action', 'id', 'pinned'])) {
    return validId(value.id) && typeof value.pinned === 'boolean';
  }
  if (value.action === 'move_tab' && hasExactKeys(value, ['version', 'action', 'id', 'index'])) {
    return validId(value.id) && Number.isSafeInteger(value.index) && (value.index as number) >= 0 && (value.index as number) < 256;
  }
  if (value.action === 'show_surface' && hasExactKeys(value, ['version', 'action', 'value'])) {
    return ['browse', 'start', 'library', 'settings', 'downloads'].includes(value.value as string);
  }
  if (['query_omnibox', 'query_library', 'set_theme', 'set_search_engine', 'clear_history'].includes(value.action)
    && hasExactKeys(value, ['version', 'action', 'value']) && typeof value.value === 'string' && value.value.length <= MAX_URL_LENGTH) {
    if (value.action === 'set_theme') return ['system', 'light', 'dark'].includes(value.value);
    if (value.action === 'set_search_engine') return ['duckduckgo', 'google', 'bing'].includes(value.value);
    if (value.action === 'clear_history') return ['hour', 'day', 'week', 'all'].includes(value.value);
    return true;
  }
  if (['set_restore_session', 'set_clear_history_on_exit'].includes(value.action)
    && hasExactKeys(value, ['version', 'action', 'enabled'])) return typeof value.enabled === 'boolean';
  if (hasExactKeys(value, ['version', 'action']) && ['bookmark_current', 'import_bookmarks', 'export_bookmarks'].includes(value.action)) return true;
  if (hasExactKeys(value, ['version', 'action', 'id']) && ['remove_visit', 'delete_bookmark', 'remove_shortcut'].includes(value.action)) return validId(value.id);
  if (value.action === 'create_folder' && hasExactKeys(value, ['version', 'action', 'title', 'parent_id'])) {
    return validTitle(value.title) && (value.parent_id === null || validId(value.parent_id));
  }
  if (value.action === 'move_bookmark' && hasExactKeys(value, ['version', 'action', 'id', 'parent_id'])) {
    return validId(value.id) && (value.parent_id === null || validId(value.parent_id));
  }
  if (value.action === 'add_shortcut' && hasExactKeys(value, ['version', 'action', 'title', 'url'])) {
    return validTitle(value.title) && validUrl(value.url);
  }
  if (value.action === 'find_page' && hasExactKeys(value, ['version', 'action', 'value', 'backwards'])) {
    return typeof value.value === 'string' && value.value.length <= 512 && typeof value.backwards === 'boolean';
  }
  if (value.action === 'zoom_page' && hasExactKeys(value, ['version', 'action', 'value'])) return ['in', 'out', 'reset'].includes(value.value as string);
  if (hasExactKeys(value, ['version', 'action']) && ['toggle_fullscreen', 'print_page', 'save_pdf'].includes(value.action)) return true;
  if (hasExactKeys(value, ['version', 'action', 'id']) && ['reveal_download', 'cancel_download'].includes(value.action)) return validId(value.id);
  if (hasExactKeys(value, ['version', 'action']) && ['clear_downloads', 'choose_download_folder', 'reset_download_folder'].includes(value.action)) return true;
  if (value.action === 'decide_download' && hasExactKeys(value, ['version', 'action', 'id', 'approve'])) {
    return validId(value.id) && typeof value.approve === 'boolean';
  }
  return false;
}

function validId(value: unknown) {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function isSnapshot(value: unknown): value is ChromeSnapshot {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['version', 'url', 'title', 'loading', 'can_back', 'can_forward', 'status', 'secure', 'tabs', 'selected_tab', 'can_restore', 'surface', 'settings', 'suggestions', 'history', 'bookmarks', 'shortcuts', 'bookmarked', 'notice', 'downloads', 'page_tools', 'download_approval_count', 'download_folder'])
    && value.version === 1
    && typeof value.url === 'string' && value.url.length <= MAX_URL_LENGTH
    && typeof value.title === 'string' && value.title.length <= MAX_TITLE_LENGTH
    && typeof value.loading === 'boolean'
    && typeof value.can_back === 'boolean'
    && typeof value.can_forward === 'boolean'
    && typeof value.status === 'string' && STATUSES.has(value.status as ChromeSnapshot['status'])
    && typeof value.secure === 'boolean'
    && Array.isArray(value.tabs) && value.tabs.length <= 256 && value.tabs.every(isTabSnapshot) && hasUniqueTabIds(value.tabs)
    && Number.isSafeInteger(value.selected_tab) && (value.selected_tab as number) >= 0
    && (value.tabs.length === 0
      ? value.selected_tab === 0
      : value.tabs.some((tab) => tab.id === value.selected_tab))
    && typeof value.can_restore === 'boolean'
    && ['browse', 'start', 'library', 'settings', 'downloads'].includes(value.surface as string)
    && (value.surface === 'downloads' || (Array.isArray(value.downloads) && value.downloads.length === 0))
    && isSettings(value.settings)
    && Array.isArray(value.suggestions) && value.suggestions.length <= 20 && value.suggestions.every(isSuggestion) && hasUniqueBy(value.suggestions, (entry) => entry.url)
    && Array.isArray(value.history) && value.history.length <= 500 && value.history.every(isHistoryEntry) && hasUniqueBy(value.history, (entry) => entry.id)
    && Array.isArray(value.bookmarks) && value.bookmarks.length <= 500 && value.bookmarks.every(isBookmarkEntry) && hasUniqueBy(value.bookmarks, (entry) => entry.id)
    && Array.isArray(value.shortcuts) && value.shortcuts.length <= 64 && value.shortcuts.every(isShortcutEntry) && hasUniqueBy(value.shortcuts, (entry) => entry.id)
    && typeof value.bookmarked === 'boolean'
    && typeof value.notice === 'string' && value.notice.length <= MAX_TITLE_LENGTH * 2
    && Array.isArray(value.downloads) && value.downloads.length <= 256 && value.downloads.every(isDownload) && hasUniqueBy(value.downloads, (entry) => entry.id)
    && (value.download_folder === null || (typeof value.download_folder === 'string' && validText(value.download_folder) && !value.download_folder.includes('/')))
    && isPageTools(value.page_tools)
    && Number.isSafeInteger(value.download_approval_count) && (value.download_approval_count as number) >= 0 && (value.download_approval_count as number) <= 16;
}

function isDownload(value: unknown): value is DownloadEntry {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['id', 'name', 'host', 'status', 'bytes']) && validId(value.id)
    && validText(value.name) && typeof value.host === 'string' && new TextEncoder().encode(value.host).length < 256
    && ['awaiting_approval', 'downloading', 'finished', 'failed', 'cancelled'].includes(value.status as string)
    && Number.isSafeInteger(value.bytes) && (value.bytes as number) >= 0;
}

function isPageTools(value: unknown): value is PageToolsState {
  if (!isObject(value) || !hasExactKeys(value, ['zoom_percent', 'find_found', 'fullscreen'])) return false;
  return Number.isSafeInteger(value.zoom_percent) && (value.zoom_percent as number) >= 25 && (value.zoom_percent as number) <= 500
    && (value.find_found === null || typeof value.find_found === 'boolean') && typeof value.fullscreen === 'boolean';
}

function isSettings(value: unknown): value is BrowserSettings {
  if (!isObject(value) || !hasExactKeys(value, ['theme', 'search_engine', 'restore_session', 'clear_history_on_exit'])) return false;
  return ['system', 'light', 'dark'].includes(value.theme as string)
    && ['duckduckgo', 'google', 'bing'].includes(value.search_engine as string)
    && typeof value.restore_session === 'boolean'
    && typeof value.clear_history_on_exit === 'boolean';
}

function isSuggestion(value: unknown): value is LocalSuggestion {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['url', 'title', 'bookmarked']) && validUrl(value.url)
    && validText(value.title) && typeof value.bookmarked === 'boolean';
}

function isHistoryEntry(value: unknown): value is HistoryEntry {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['id', 'url', 'title', 'visited_at_ms']) && validId(value.id)
    && validUrl(value.url) && validText(value.title)
    && Number.isSafeInteger(value.visited_at_ms) && (value.visited_at_ms as number) >= 0 && (value.visited_at_ms as number) <= 8_640_000_000_000_000;
}

function isBookmarkEntry(value: unknown): value is BookmarkEntry {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['id', 'parent_id', 'url', 'title']) && validId(value.id)
    && (value.parent_id === null || validId(value.parent_id))
    && (value.url === null || validUrl(value.url)) && validText(value.title);
}

function isShortcutEntry(value: unknown): value is ShortcutEntry {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['id', 'url', 'title']) && validId(value.id) && validUrl(value.url) && validText(value.title);
}

function validText(value: unknown) {
  return typeof value === 'string' && value.length <= MAX_TITLE_LENGTH;
}

function validTitle(value: unknown) {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= MAX_TITLE_LENGTH;
}

function validUrl(value: unknown) {
  return typeof value === 'string' && value.length > 0 && value.length <= MAX_URL_LENGTH;
}

function hasUniqueBy<T>(rows: T[], key: (row: T) => string | number) {
  return new Set(rows.map(key)).size === rows.length;
}

function freezeRows<T extends object>(rows: T[]) {
  return Object.freeze(rows.map((row) => Object.freeze({ ...row }))) as unknown as T[];
}

function hasUniqueTabIds(tabs: TabSnapshot[]) {
  return new Set(tabs.map((tab) => tab.id)).size === tabs.length;
}

function isTabSnapshot(value: unknown): value is TabSnapshot {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['id', 'title', 'pinned', 'loading', 'discarded'])
    && validId(value.id)
    && typeof value.title === 'string' && value.title.length <= MAX_TITLE_LENGTH
    && typeof value.pinned === 'boolean'
    && typeof value.loading === 'boolean'
    && typeof value.discarded === 'boolean';
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, expected: string[]) {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

declare global {
  interface Window {
    muffinChrome?: ChromeBridge;
  }
}

export function installChromeBridge() {
  const state = createChromeBridge(() => {
    const addressInput = document.querySelector<HTMLInputElement>('#address');
    addressInput?.focus();
    addressInput?.select();
  }, () => {
    window.dispatchEvent(new Event('muffin:focus-tab-search'));
  }, () => {
    window.dispatchEvent(new Event('muffin:focus-find'));
  });

  Object.defineProperty(window, 'muffinChrome', {
    value: state.bridge,
    configurable: false,
    enumerable: false,
    writable: false
  });
  window.addEventListener('muffin:action', (event) => {
    const detail = (event as CustomEvent<unknown>).detail;
    if (isAction(detail)) state.enqueue(detail);
  });
  return state;
}

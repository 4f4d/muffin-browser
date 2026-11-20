export type ChromeAction =
  | { version: 1; action: 'navigate'; value: string }
  | {
      version: 1;
      action: 'back' | 'forward' | 'reload' | 'stop' | 'focus_page' | 'open_home';
    }
  | { version: 1; action: 'new_tab' | 'new_window' | 'reopen_tab' | 'tab_search' }
  | { version: 1; action: 'select_tab' | 'close_tab' | 'duplicate_tab' | 'discard_tab'; id: number }
  | { version: 1; action: 'pin_tab'; id: number; pinned: boolean }
  | { version: 1; action: 'move_tab'; id: number; index: number };

export type TabSnapshot = {
  id: number;
  title: string;
  pinned: boolean;
  loading: boolean;
  discarded: boolean;
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
};

export type ChromeBridge = {
  take(): ChromeAction | null;
  render(snapshot: ChromeSnapshot): void;
  focusAddress(): void;
  focusTabSearch(): void;
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
  can_restore: false
};

export function createChromeBridge(
  focusAddress: () => void = () => undefined,
  focusTabSearch: () => void = () => undefined
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
        tabs: Object.freeze(snapshot.tabs.map((tab) => Object.freeze({ ...tab }))) as unknown as TabSnapshot[]
      });
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('muffin:render', { detail: currentSnapshot }));
      }
    },
    focusAddress,
    focusTabSearch,
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
  return false;
}

function validId(value: unknown) {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function isSnapshot(value: unknown): value is ChromeSnapshot {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['version', 'url', 'title', 'loading', 'can_back', 'can_forward', 'status', 'secure', 'tabs', 'selected_tab', 'can_restore'])
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
    && typeof value.can_restore === 'boolean';
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

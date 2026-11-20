export type ChromeAction =
  | { version: 1; action: 'navigate'; value: string }
  | {
      version: 1;
      action: 'back' | 'forward' | 'reload' | 'stop' | 'focus_page' | 'open_home';
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
};

export type ChromeBridge = {
  take(): ChromeAction | null;
  render(snapshot: ChromeSnapshot): void;
  focusAddress(): void;
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
  secure: false
};

export function createChromeBridge(focusAddress: () => void = () => undefined) {
  const actions: ChromeAction[] = [];
  let currentSnapshot = initialSnapshot;

  const bridge: ChromeBridge = {
    take() {
      return actions.shift() ?? null;
    },
    render(snapshot) {
      if (!isSnapshot(snapshot)) return;
      currentSnapshot = Object.freeze({ ...snapshot });
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('muffin:render', { detail: currentSnapshot }));
      }
    },
    focusAddress,
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
  return hasExactKeys(value, ['version', 'action'])
    && ['back', 'forward', 'reload', 'stop', 'focus_page', 'open_home'].includes(value.action);
}

function isSnapshot(value: unknown): value is ChromeSnapshot {
  if (!isObject(value)) return false;
  return hasExactKeys(value, ['version', 'url', 'title', 'loading', 'can_back', 'can_forward', 'status', 'secure'])
    && value.version === 1
    && typeof value.url === 'string' && value.url.length <= MAX_URL_LENGTH
    && typeof value.title === 'string' && value.title.length <= MAX_TITLE_LENGTH
    && typeof value.loading === 'boolean'
    && typeof value.can_back === 'boolean'
    && typeof value.can_forward === 'boolean'
    && typeof value.status === 'string' && STATUSES.has(value.status as ChromeSnapshot['status'])
    && typeof value.secure === 'boolean';
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

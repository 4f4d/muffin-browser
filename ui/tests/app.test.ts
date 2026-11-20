import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { mount, tick, unmount } from 'svelte';
import App from '../src/routes/App.svelte';
import { installChromeBridge } from '../src/lib/bridge';

describe('browser chrome controls', () => {
  let component: ReturnType<typeof mount>;
  let bridgeState: ReturnType<typeof installChromeBridge>;

  beforeAll(() => {
    bridgeState = installChromeBridge();
    component = mount(App, { target: document.body });
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
    bridgeState.bridge.render({
      version: 1,
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
    });
    await tick();
    const reload = document.querySelector<HTMLButtonElement>('.reload')!;
    expect(reload.getAttribute('aria-label')).toBe('Stop loading');
    reload.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'stop' });
    expect(document.querySelector('#address')?.textContent).not.toContain('<unsafe title>');
  });

  it('preserves a blocked submission until the address edit is canceled', async () => {
    const oldAddress = 'https://example.test';
    const snapshot = {
      version: 1 as const,
      url: oldAddress,
      title: 'First page',
      loading: false,
      can_back: true,
      can_forward: false,
      secure: true,
      tabs: [{ id: 20, title: 'First page', pinned: false, loading: false, discarded: false }],
      selected_tab: 20,
      can_restore: false
    };
    bridgeState.bridge.render({ ...snapshot, status: 'ready' });
    await tick();
    const input = document.querySelector<HTMLInputElement>('#address')!;
    input.focus();
    input.value = 'file:///unavailable';
    input.dispatchEvent(new Event('input', { bubbles: true }));
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

  it('routes tab lifecycle actions through the typed queue', async () => {
    const tabs = [
      { id: 10, title: 'Mail', pinned: true, loading: false, discarded: false },
      { id: 20, title: 'Reading list', pinned: false, loading: false, discarded: false },
      { id: 30, title: 'Docs home', pinned: false, loading: false, discarded: true }
    ];
    bridgeState.bridge.render({
      version: 1,
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
    });
    await tick();

    document.querySelector<HTMLButtonElement>('.new-tab-button')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'new_tab' });
    document.querySelector<HTMLButtonElement>('[aria-label="Open new window"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'new_window' });
    document.querySelector<HTMLButtonElement>('[aria-label="Restore recently closed tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'reopen_tab' });
    document.querySelector<HTMLButtonElement>('[aria-label="Pin selected tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'pin_tab', id: 20, pinned: true });
    document.querySelector<HTMLButtonElement>('[aria-label="Duplicate selected tab"]')!.click();
    expect(bridgeState.bridge.take()).toEqual({ version: 1, action: 'duplicate_tab', id: 20 });
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
  });

  it('supports keyboard tab reorder and inline tab search', async () => {
    bridgeState.bridge.render({
      version: 1,
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
    });
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
    bridgeState.bridge.render({
      version: 1,
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
    });
    await tick();
    document.querySelector<HTMLInputElement>('#tab-search')!.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
    );
    await tick();
    await tick();
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Search tabs');
  });
});

async function openTabSearchFromNative() {
  window.muffinChrome?.focusTabSearch();
  await tick();
  await tick();
}

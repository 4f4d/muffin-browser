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
      secure: true
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
      secure: true
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
});

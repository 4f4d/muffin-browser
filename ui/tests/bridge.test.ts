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
      can_restore: false
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
});

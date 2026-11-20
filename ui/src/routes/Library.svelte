<script lang="ts">
  import type { BookmarkEntry, ChromeAction, ChromeSnapshot } from '../lib/bridge';

  let { snapshot, send }: { snapshot: ChromeSnapshot; send: (action: ChromeAction) => void } = $props();
  let section = $state<'history' | 'bookmarks'>('history');
  let historyRange = $state<'all' | 'today' | 'week'>('all');
  let libraryQuery = $state('');
  let parentId = $state<number | null>(null);
  let folderFormOpen = $state(false);
  let folderTitle = $state('');
  let clearRange = $state<'hour' | 'day' | 'week' | 'all'>('week');
  let confirmClear = $state(false);

  let folder = $derived(snapshot.bookmarks.find((entry) => entry.id === parentId && entry.url === null));
  let folders = $derived(snapshot.bookmarks.filter((entry) => entry.url === null));
  let visibleHistory = $derived(snapshot.history.filter((entry) => {
    const queryMatches = `${entry.title} ${entry.url}`.toLocaleLowerCase().includes(libraryQuery.trim().toLocaleLowerCase());
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const cutoff = historyRange === 'today' ? today.getTime()
      : historyRange === 'week' ? Date.now() - 7 * 24 * 60 * 60 * 1000 : 0;
    return queryMatches && entry.visited_at_ms >= cutoff;
  }));
  let visibleBookmarks = $derived(snapshot.bookmarks.filter((entry) =>
    entry.parent_id === parentId
    && `${entry.title} ${entry.url ?? ''}`.toLocaleLowerCase().includes(libraryQuery.trim().toLocaleLowerCase())
  ));

  function queryChanged() {
    send({ version: 1, action: 'query_library', value: libraryQuery });
  }

  function openUrl(url: string) {
    send({ version: 1, action: 'show_surface', value: 'browse' });
    send({ version: 1, action: 'navigate', value: url });
  }

  function createFolder(event: SubmitEvent) {
    event.preventDefault();
    const title = folderTitle.trim();
    if (!title) return;
    send({ version: 1, action: 'create_folder', title, parent_id: parentId });
    folderTitle = '';
    folderFormOpen = false;
  }

  function moveEntry(entry: BookmarkEntry, target: string) {
    const nextParent = target === 'root' ? null : Number(target);
    if (nextParent === entry.parent_id) return;
    send({ version: 1, action: 'move_bookmark', id: entry.id, parent_id: nextParent });
  }

  function isDescendant(folderId: number, ancestorId: number) {
    let current = snapshot.bookmarks.find((entry) => entry.id === folderId);
    const seen = new Set<number>();
    while (current && current.parent_id !== null && !seen.has(current.id)) {
      if (current.parent_id === ancestorId) return true;
      seen.add(current.id);
      current = snapshot.bookmarks.find((entry) => entry.id === current?.parent_id);
    }
    return false;
  }

  function clearHistory() {
    send({ version: 1, action: 'clear_history', value: clearRange });
    confirmClear = false;
  }
</script>

<section class="surface-page library-page" aria-labelledby="library-heading">
  <div class="page-width">
    {#if snapshot.notice}<p class="surface-notice" role="status" aria-live="polite">{snapshot.notice}</p>{/if}
    <header class="page-title-row">
      <h1 id="library-heading">Library</h1>
      <div class="library-actions">
        {#if section === 'bookmarks'}
          <button class="quiet-button" type="button" disabled={!snapshot.url || snapshot.bookmarked} onclick={() => send({ version: 1, action: 'bookmark_current' })}>{snapshot.bookmarked ? 'Already saved' : 'Bookmark current page'}</button>
          <button class="quiet-button" type="button" onclick={() => folderFormOpen = !folderFormOpen}>{folderFormOpen ? 'Cancel folder' : 'New folder'}</button>
          <button class="quiet-button" type="button" aria-label="Import bookmarks" onclick={() => send({ version: 1, action: 'import_bookmarks' })}>Import</button>
          <button class="quiet-button" type="button" aria-label="Export bookmarks" disabled={snapshot.bookmarks.length === 0} onclick={() => send({ version: 1, action: 'export_bookmarks' })}>Export</button>
        {:else}
          <select class="range-select" aria-label="History range to clear" bind:value={clearRange}>
            <option value="hour">Past hour</option>
            <option value="day">Past day</option>
            <option value="week">Past week</option>
            <option value="all">All history</option>
          </select>
          <button class="quiet-button" type="button" onclick={() => confirmClear = true}>Clear history</button>
        {/if}
      </div>
    </header>

    <div class="library-nav" role="group" aria-label="Library sections">
      <button aria-pressed={section === 'history'} class:current={section === 'history'} onclick={() => section = 'history'}>History</button>
      <button aria-pressed={section === 'bookmarks'} class:current={section === 'bookmarks'} onclick={() => section = 'bookmarks'}>Bookmarks</button>
    </div>

    {#if confirmClear}
      <section class="confirm-row" aria-label="Confirm history deletion">
        <p>Clear {clearRange === 'all' ? 'all history' : `history from the past ${clearRange}`}?</p>
        <button class="text-action danger-action" type="button" onclick={clearHistory}>Clear history</button>
        <button class="quiet-button" type="button" onclick={() => confirmClear = false}>Cancel</button>
      </section>
    {/if}

    {#if section === 'history'}
      <div class="list-toolbar">
        <div class="filter-group" role="group" aria-label="Filter history by time">
          <button aria-pressed={historyRange === 'all'} onclick={() => historyRange = 'all'}>All</button>
          <button aria-pressed={historyRange === 'today'} onclick={() => historyRange = 'today'}>Today</button>
          <button aria-pressed={historyRange === 'week'} onclick={() => historyRange = 'week'}>7 days</button>
        </div>
        <label class="library-search">
          <span class="visually-hidden">Search history</span>
          <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m12.5 12.5 4 4" /></svg>
          <input bind:value={libraryQuery} oninput={queryChanged} placeholder="Search history" />
        </label>
      </div>

      {#if visibleHistory.length > 0}
        <ul class="library-list history-list" aria-label="Browsing history">
          {#each visibleHistory as entry (entry.id)}
            <li>
              <button class="library-open" type="button" onclick={() => openUrl(entry.url)}>
                <span class="library-item-title">{entry.title}</span>
                <span class="library-item-url">{entry.url}</span>
              </button>
              <time datetime={new Date(entry.visited_at_ms).toISOString()}>{new Date(entry.visited_at_ms).toLocaleString()}</time>
              <button class="row-remove" type="button" aria-label={`Remove ${entry.title} from history`} title="Remove visit" onclick={() => send({ version: 1, action: 'remove_visit', id: entry.id })}>
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="empty-state"><p>{snapshot.history.length === 0 ? 'Pages you visit will appear here.' : 'No pages match this filter.'}</p><button class="text-action" type="button" onclick={() => send({ version: 1, action: 'show_surface', value: 'start' })}>Open start page</button></div>
      {/if}
    {:else}
      <div class="list-toolbar">
        <div class="folder-path">
          {#if parentId !== null}
            <button class="text-action" type="button" onclick={() => parentId = folder?.parent_id ?? null}>{folder?.parent_id === null ? 'All bookmarks' : 'Parent folder'}</button>
            <span aria-hidden="true">/</span>
            <span>{folder?.title ?? 'Folder'}</span>
          {:else}
            <span>All bookmarks</span>
          {/if}
        </div>
        <label class="library-search">
          <span class="visually-hidden">Search bookmarks</span>
          <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m12.5 12.5 4 4" /></svg>
          <input bind:value={libraryQuery} oninput={queryChanged} placeholder="Search bookmarks" />
        </label>
      </div>

      {#if folderFormOpen}
        <form class="inline-form folder-form" onsubmit={createFolder}>
          <label><span>Folder name</span><input bind:value={folderTitle} maxlength="512" required /></label>
          <button class="text-action" type="submit">Create folder</button>
        </form>
      {/if}

      {#if visibleBookmarks.length > 0}
        <ul class="library-list bookmark-list" aria-label={folder?.title ?? 'Bookmarks'}>
          {#each visibleBookmarks as entry (entry.id)}
            <li class:folder-row={entry.url === null}>
              {#if entry.url === null}
                <button class="library-open folder-open" type="button" onclick={() => parentId = entry.id}>
                  <span class="folder-symbol" aria-hidden="true"><svg viewBox="0 0 20 20"><path d="M2.5 5.5A1.5 1.5 0 0 1 4 4h4l2 2h6A1.5 1.5 0 0 1 17.5 7.5v8A1.5 1.5 0 0 1 16 17H4a1.5 1.5 0 0 1-1.5-1.5z" /></svg></span>
                  <span class="library-item-title">{entry.title}</span>
                  <span class="folder-count">Open folder</span>
                </button>
              {:else}
                <button class="library-open" type="button" onclick={() => openUrl(entry.url!)}>
                  <span class="library-item-title">{entry.title}</span>
                  <span class="library-item-url">{entry.url}</span>
                </button>
              {/if}
              <label class="move-bookmark">
                <span class="visually-hidden">Move {entry.title} to folder</span>
                <select aria-label={`Move ${entry.title} to folder`} value={entry.parent_id === null ? 'root' : String(entry.parent_id)} onchange={(event) => moveEntry(entry, event.currentTarget.value)}>
                  <option value="root">All bookmarks</option>
                  {#each folders as destination (destination.id)}
                    {#if destination.id !== entry.id && !isDescendant(destination.id, entry.id)}
                      <option value={String(destination.id)}>{destination.title}</option>
                    {/if}
                  {/each}
                </select>
              </label>
              <button class="row-remove" type="button" aria-label={`Delete ${entry.title}`} title="Delete bookmark" onclick={() => send({ version: 1, action: 'delete_bookmark', id: entry.id })}>
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 4.5 7 7m0-7-7 7" /></svg>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="empty-state">
          <p>{snapshot.bookmarks.length === 0 ? 'Saved pages and folders will appear here.' : 'This folder has no saved pages.'}</p>
          <button class="text-action" type="button" disabled={!snapshot.url || snapshot.bookmarked} onclick={() => send({ version: 1, action: 'bookmark_current' })}>{snapshot.bookmarked ? 'Page already saved' : 'Save current page'}</button>
        </div>
      {/if}
    {/if}
  </div>
</section>

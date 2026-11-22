<script lang="ts">
  type IconName =
    | 'back' | 'forward' | 'reload' | 'home' | 'download' | 'more' | 'plus' | 'search'
    | 'page-tools' | 'pin' | 'unpin' | 'duplicate' | 'restore' | 'window' | 'library' | 'settings';
  let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
  {#if name === 'back'}
    <path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>
  {:else if name === 'forward'}
    <path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
  {:else if name === 'reload'}
    <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>
  {:else if name === 'home'}
    <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
  {:else if name === 'download'}
    <path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>
  {:else if name === 'more'}
    <circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>
  {:else if name === 'plus'}
    <path d="M5 12h14"/><path d="M12 5v14"/>
  {:else if name === 'search'}
    <path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>
  {:else if name === 'page-tools'}
    <path d="M10 5H3 M12 19H3 M14 3v4 M16 17v4 M21 12h-9 M21 19h-5 M21 5h-7 M8 10v4 M8 12H3"/>
  {:else if name === 'pin'}
    <path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>
  {:else if name === 'unpin'}
    <path d="M12 17v5 M15 9.34V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H7.89 M2 2l20 20"/>
    <path d="M9 9v1.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h11"/>
  {:else if name === 'duplicate'}
    <rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>
  {:else if name === 'restore'}
    <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8 M3 3v5h5"/>
  {:else if name === 'window'}
    <rect x="2" y="4" width="20" height="16" rx="2"/><path d="M10 4v4 M2 8h20 M6 4v4"/>
  {:else if name === 'library'}
    <path d="M16 6l4 14 M12 6v14 M8 8v12 M4 4v16"/>
  {:else if name === 'settings'}
    <path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>
  {/if}
</svg>

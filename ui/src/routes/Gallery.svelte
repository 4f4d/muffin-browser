<script lang="ts">
  import ChromeIcon from '../lib/ChromeIcon.svelte';

  const exampleAddress = 'example.com';
  let address = $state(exampleAddress);
</script>

<main class="gallery-page">
  <header class="gallery-header">
    <div>
      <div class="eyebrow">MUFFIN / CHROME</div>
      <h1>Browser chrome</h1>
    </div>
  </header>

  <section class="gallery-section" aria-labelledby="toolbar-heading">
    <div class="section-heading">
      <h2 id="toolbar-heading">Navigation toolbar</h2>
    </div>
    <div class="sample-stage">
      <div class="toolbar sample-toolbar">
        <nav class="history" aria-label="Page history">
          <button class="icon-button" aria-label="Go back" disabled>
            <ChromeIcon name="back" />
          </button>
          <button class="icon-button" aria-label="Go forward" disabled>
            <ChromeIcon name="forward" />
          </button>
          <button class="icon-button" aria-label="Reload page">
            <ChromeIcon name="reload" />
          </button>
        </nav>
        <div class="address-shell sample-address">
          <span class="security-mark secure" aria-hidden="true"><svg viewBox="0 0 20 20"><rect x="4.6" y="8.6" width="10.8" height="8" rx="2" /><path d="M6.8 8.5V6a3.2 3.2 0 1 1 6.4 0v2.5" /></svg></span>
          <label class="visually-hidden" for="gallery-address">Search or enter address</label>
          <input id="gallery-address" bind:value={address} aria-label="Search or enter address" />
        </div>
        <div class="toolbar-actions">
          <button type="button" class="toolbar-action" aria-label="Open start page" onclick={() => address = exampleAddress}>
            <ChromeIcon name="home" />
          </button>
        </div>
      </div>
    </div>
  </section>

  <section class="gallery-section" aria-labelledby="tokens-heading">
    <div class="section-heading"><h2 id="tokens-heading">Material and states</h2><span>System appearance</span></div>
    <div class="swatches">
      <div class="swatch"><i class="swatch-porcelain"></i><span>Porcelain</span><code>#F5F6F7</code></div>
      <div class="swatch"><i class="swatch-frost"></i><span>Frost</span><code>#FBFCFD</code></div>
      <div class="swatch"><i class="swatch-graphite"></i><span>Graphite</span><code>#202327</code></div>
      <div class="swatch"><i class="swatch-slate"></i><span>Slate</span><code>#353A40</code></div>
      <div class="swatch"><i class="swatch-amber"></i><span>Amber</span><code>#C58B35</code></div>
    </div>
    <div class="state-row"><span class="state-chip"><i class="dot ready"></i>Ready</span><span class="state-chip"><i class="dot loading"></i>Loading</span><span class="state-chip"><i class="dot blocked"></i>Blocked</span><span class="state-chip secure-chip">Secure connection</span></div>
  </section>
</main>

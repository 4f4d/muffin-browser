<script lang="ts">
  let address = $state('example.com');
</script>

<main class="gallery-page">
  <header class="gallery-header">
    <div>
      <div class="eyebrow">MUFFIN / CHROME</div>
      <h1>Browser chrome</h1>
    </div>
    <p>56 px host region · native titlebar · responsive controls</p>
  </header>

  <section class="gallery-section" aria-labelledby="toolbar-heading">
    <div class="section-heading">
      <h2 id="toolbar-heading">Navigation toolbar</h2>
      <span>Light surface</span>
    </div>
    <div class="sample-stage">
      <div class="toolbar sample-toolbar">
        <nav class="history" aria-label="Page history">
          <button class="icon-button" aria-label="Go back" disabled>
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M8 4 2.8 9.2 8 14.4M3.2 9.2h8.1a5.1 5.1 0 0 1 0 10.2H8.8" /></svg>
          </button>
          <button class="icon-button" aria-label="Go forward" disabled>
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12 4 5.2 5.2-5.2 5.2m4.8-5.2H8.7a5.1 5.1 0 0 0 0 10.2h2.5" /></svg>
          </button>
          <button class="icon-button" aria-label="Reload page">
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M16.3 8a6.4 6.4 0 1 0 .1 3.2M16.2 3.8v4.5h-4.5" /></svg>
          </button>
        </nav>
        <div class="address-shell sample-address">
          <span class="security-mark secure" aria-hidden="true"><svg viewBox="0 0 20 20"><rect x="4.6" y="8.6" width="10.8" height="8" rx="2" /><path d="M6.8 8.5V6a3.2 3.2 0 1 1 6.4 0v2.5" /></svg></span>
          <label class="visually-hidden" for="gallery-address">Search or enter address</label>
          <input id="gallery-address" bind:value={address} aria-label="Search or enter address" />
        </div>
        <button class="home-button" aria-label="Open start page"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m3.5 9 6.5-5.5L16.5 9v7a1.5 1.5 0 0 1-1.5 1.5h-3.2v-5h-3.6v5H5A1.5 1.5 0 0 1 3.5 16z" /></svg></button>
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

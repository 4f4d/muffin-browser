# S5 comparative macOS baselines

The corrected run recorded all 40 launches on the same Apple M4 arm64 Mac, macOS 15.8.3 (24H241), on AC power. It compared the optimized WRY 0.57.2 / TAO 0.37.2 fixture backed by WKWebView 20621.3.11.11.3 with installed Chrome 157.0.8103.79 and Firefox 159.0.1. Each workload has two first-process and two repeat-process samples. The WRY 0-tab row contains a native TAO window with no WebView. All other rows required a distinct load beacon from every requested slot. [Identity, method and raw samples](evidence/continuation-s5-macos-raw.json), [derived medians and component breakdown](evidence/continuation-s5-macos-derived.json)

| Fixture or browser | Tabs | First / repeat readiness (ms) | Repeat host / observed total (MiB) | Repeat processes | Repeat short CPU (% of one core) |
|---|---:|---:|---:|---:|---:|
| WRY + TAO | 0 | 89 / 89 | 13.8 / 13.8 | 1 | 0.02 |
| WRY + TAO | 1 | 238 / 210 | 19.5 / 107.3 | 4 | 1.56 |
| WRY + TAO | 5 | 266 / 273 | 20.4 / 254.2 | 8 | 1.73 |
| WRY + TAO | 10 | 370 / 371 | 21.1 / 460.8 | 13 | 8.65 |
| Chrome | 1 | 395 / 317 | 66.3 / 211.7 | 7 | 4.41 |
| Chrome | 5 | 356 / 348 | 72.5 / 287.9 | 11 | 5.82 |
| Chrome | 10 | 407 / 394 | 81.8 / 373.4 | 16 | 6.54 |
| Firefox | 1 | 1056 / 987 | 403.0 / 1064.6 | 11 | 80.86 |
| Firefox | 5 | 1101 / 1095 | 376.7 / 1126.9 | 14 | 74.67 |
| Firefox | 10 | 1172 / 1088 | 365.8 / 1126.0 | 14 | 84.23 |

The working desktop was warm. The first-process column is a new process and disposable comparator profile. The repeat column launches a fresh process with the same profile. WRY used a dedicated test data store across launches. Time measures shell construction at zero tabs and receipt of the last JS load beacon otherwise. No first paint, boot-cold start or long-run idle measurement was attempted. A browser snapshot began 1.5 seconds after readiness and the second followed about one second later. The Firefox results include LaunchServices launch overhead. The short CPU percentages include startup work and do not predict steady-state power use. [Raw timings and process samples](evidence/continuation-s5-macos-raw.json)

Physical footprint and RSS were recorded separately with Darwin proc_pid_rusage. Browser app processes and descendants are separated from WRY's launch-associated, launchd-parented WebKit helpers. The helper association is estimated by new PIDs and names. The totals are summed reported footprints, with shared-page and concurrent-helper attribution limits. The derived records retain host, renderer, GPU, network and other buckets. An unclassified role contributes to other and a zero bucket does not establish absence. The corrected recorder uses the Mach timebase (125/3 on this host). The pilot retained separately mislabeled raw CPU ticks and direct Firefox launches exited without readiness. The converter was verified against a local CPU counter, and the Firefox retry used an explicitly isolated profile. Safari WebDriver returned session-not-created because remote automation was disabled. No setting was changed. [ABI check](evidence/continuation-s5-rusage-layout.txt), [pilot](evidence/continuation-s5-pilot-raw.json), [calibration and browser route evidence](evidence/continuation-s5-browser-alternatives.txt)

The prior WKWebView floor measured median 376 ms to its different readiness event and about 55.6–56.6 MiB including associated helpers. It ran on battery with one nonpersistent view. Direct subtraction would mix profile, power and page differences. Within the current run WRY's host remained about 14–21 MiB, while total footprint increased with live views. The ten-tab repeat median was 460.8 MiB including attributed helpers versus 373.4 MiB for this Chrome configuration. The small samples do not establish a general browser ranking. Tab renderer lifecycle and the total active WebView count matter more to memory than the host binary size. [Earlier floor](evidence/engine-floor-macos.json), [calibrated benchmark](evidence/continuation-s5-macos-derived.json)

The optimized fixture executable was 1,588,000 bytes, its ZIP 615,895 bytes and its bundle 1,556 KiB in allocated blocks. The archive excludes OS frameworks and browser profile data. The artifact records the system dynamic links and local comparator app directory sizes. These are diagnostic footprint measurements. Shipping package and installer requirements remain separate. [Footprint transcript](evidence/continuation-s5-macos-footprint.txt)

Windows and both desktop Linux rows had no valid interactive bare-metal comparison environment. Their measurements are unrun. Previous Server and headless artifacts remain useful for functional coverage. [Environment inventory](evidence/continuation-environment.txt)

# Spreadsheet application comparison

Compare the production React/TypeScript jotai-holo spreadsheet with this
Rust/WASM/Yew adaptation on the same machine, Chromium binary and CSV bytes.
The latest recorded run is in [RESULTS.md](RESULTS.md), generated from
[raw samples and artifact hashes](results/latest.json).

This measures complete applications, not isolated atom libraries or language speed.
TS restores sheets on demand using IndexedDB and registry leases. Rust eagerly
constructs the four sheets and saves whole-book snapshots to localStorage.
The formula engines, equality policies, rendering and lifecycle implementations
also differ. These differences are part of the observed timings.

## Reproduce

Build the TS comparison entry from a checkout containing the CSV sheets:

```sh
cd /path/to/jotai-holo
pnpm install --frozen-lockfile
pnpm spreadsheet:build
```

In this repository, install Rust's WASM target and Trunk as described in the root
README, then build the optimized Rust app:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cd examples/yew-spreadsheet
trunk build --release --dist ../../target/spreadsheet-comparison/rust --public-url ./
cd ../..
npm ci
npx playwright install chromium
node benchmarks/spreadsheet/compare.mjs --ts-dir=/path/to/jotai-holo/.spreadsheet-performance --samples=30 --cold=20
node benchmarks/spreadsheet/summarize.mjs
```

The runner starts two local static servers with identical no-cache headers and
no HTTP compression, checks the input SHA-256 hashes, launches one headless
Chromium instance and closes its contexts/servers on completion. It leaves the
user's browser and saved books untouched. `--out=path` selects another raw JSON
artifact; summarize it with `node benchmarks/spreadsheet/summarize.mjs path`.

## What is timed

| Metric | Start → end |
| --- | --- |
| Open book | Navigation time origin → connected gift grid with D5 = 4136 |
| First sample open | Tab click dispatch → matching A1 header in the new sheet |
| Warm sample switch | Same endpoint after both sheets have been visited |
| Plain cell edit | Enter dispatch → normalized displayed source value with readOnly restored |
| Edit with SUM | Enter dispatch on a numeric source → updated visible SUM result |

The plain edit uses C2 in Penguins and G2 in Titanic. For the formula scenario,
the runner deliberately replaces D2 with `=SUM(C2:C345)` and F2 with
`=SUM(G2:G892)`, respectively, in disposable test contexts. Both implementations
receive the same fixture changes and input values. Formula ranges have 344 and
891 source cells. Go-to navigation, double-click and draft entry are setup and
are outside the measured commit interval.

The clock is `performance.now()` inside the page. Identical instrumentation
observes input value setters and DOM mutations. It resolves only when the
expected state/value is visible in the DOM. This avoids counting Playwright/CDP
round trips, but includes the instrumentation's small overhead. It is **not**
time to paint, whole keystroke latency, model-only calculation time or an FPS test.

Cold runs use fresh storage and HTTP-cache contexts, alternating TS/Rust order.
They share one Chromium process; they are not fresh browser or OS starts.
Warm work runs sequentially in TS → Rust → Rust → TS blocks, with fresh contexts
and discarded warmups in each block. Samples are split between the two blocks
for each implementation. The run waits for each app's Saved status outside
measured intervals, so previous delayed saves do not overlap the next trial.
Synchronous serialization triggered by the current action can still affect it.

Median and p95 use nearest-rank selection from the raw samples. They describe
this run, not confidence intervals or performance guarantees. Background system
activity and JIT/GC scheduling remain uncontrolled. Browser version, host CPU,
source revisions, build-file hashes, raw values and sample counts are recorded.

## Bundle sizes

The report sums HTML/CSS/JS/WASM files in each standalone production output.
It also reports the sum of gzip-compressed files as a packaging comparison;
the measurement servers themselves do **not** gzip responses. This includes
sample data and UI dependencies, and is not the package size of Jotai/Holo.
Browser JS heap metrics are intentionally omitted because they exclude WASM
linear memory and would not be comparable.

## Validation

The TS native tests check dataset dimensions, last rows, missing values, CSV
quoting/limits, collision-safe metadata migration and reset. Browser checks cover
editing, camera, sample persistence and reset in all four documentation locales.
The Rust equivalents live in `examples/yew-spreadsheet/tests` and `tests/docs`.
The benchmark asserts input hashes, expected DOM values and absence of page errors.

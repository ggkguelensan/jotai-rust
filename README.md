# Jotai + Holo for Rust

An experimental, synchronous Rust adaptation of the atomic state model in
[Jotai](https://github.com/pmndrs/jotai) and the declarative model/view bindings in
[jotai-holo](https://github.com/ggkguelensan/jotai-holo).
This is an initial implementation, not API or behavior parity with either library.

```text
crates/jotai             atoms → store-local dependency graph → subscriptions
crates/jotai-holo        projections, actions, events → owned bonds
crates/jotai-holo-yew    declarative read/write bindings → ordinary Yew props
examples/yew-counter    one model → two isolated stores → shared counter views
```

## Run the example

Install stable Rust with rustup, then:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd examples/yew-counter
trunk serve --address 127.0.0.1 --port 8080
```

Open <http://127.0.0.1:8080>. Increment either counter in a panel; both update,
while the other store stays unchanged. Emit two identical events, disconnect and
reconnect the bond, or hide and remount the derived values. An unrelated component
shows its render count.

## Atoms and stores

```rust
use jotai::{Store, atom, derived};

let count = atom(0);
let doubled = {
    let count = count.clone();
    derived(move |get| get.get(&count) * 2)
};
let store = Store::new();
store.set(&count, 5);
assert_eq!(store.get(&doubled), 10);
```

Atom definitions have stable identities. Each store owns independent values and
caches. Derived reads are lazy, cached, and track conditional dependencies.
`get` clones a value; `get_shared` returns `Rc<T>`. Values use `PartialEq` to
suppress unchanged writes and notifications. This intentionally differs from
JavaScript's `Object.is` reference equality.

Keep a `Subscription` alive to receive changes; dropping it immediately removes
the listener. After the last subscriber leaves, derived computations wait for a
read. `Store::batch` coalesces state notifications, including nested actions,
and leaves writes immediately readable. It is not a transaction or rollback.

## Holo bindings

Views receive regular Yew props, and can be rendered directly without a store.
The adapter creates callbacks once per bound store and subscribes only to reads.
The example contains the full model, plain view and connecting component.

```rust,ignore
let holo = Holo::new(|| CounterProps {
    count: 0,
    on_increment: Callback::noop(),
})
.read(&model.count, |props, value| props.count = value)
.write(&model.increment, |props, callback| props.on_increment = callback);

// Inside the connecting component; the view itself knows nothing about atoms:
let props = use_holo(holo, store);
html! { <CounterView ..props /> }
```

Build a definition once, or retain it with `use_memo`. Finish the builder before
cloning it. `Holo::new` supplies fixed props and defaults; bindings overwrite
selected fields. Unlike TypeScript holo, this MVP does not reject duplicate
field assignments or infer remaining public props. The adapter releases read
subscriptions on unmount or store replacement and checks for writes between
render and effect setup.

## Port status

| Original concept | Current Rust implementation |
| --- | --- |
| Primitive / derived state | `atom` / `derived`; `state_atom` aliases primitive creation |
| `actionAtom` | `Action<Args, Result>`; synchronous invocation, batched writes, preserved return value |
| `eventAtom` | `Event<T>` / `event_atom`; no retained payload; equal events delivered separately |
| `ion` / `prism` | Single-source projection / selector; combine inputs with `derived` |
| `bond` | Explicit event trigger, guard, map and action target; shared connection per bond/store |
| `holo` | Typed props builder, `read` / `write`, fixed props via defaults, Yew hook |
| Model registry / lifecycle | Not implemented; demo owns model instances and bond leases explicitly |
| `holoList` / `holoMatch` / committed `input` | Not implemented |
| Async atoms, Suspense, cancellation | Not implemented |
| Query, forms, persistence, analytics | Not implemented |

Bond leases are reference counted: multiple owners cause one delivery; the last
lease disconnects. Event deliveries run FIFO after the outer action/batch ends,
and sample state at delivery time. Disposal skips queued deliveries. Connections
do not keep stores alive. Use `Store::downgrade` in callbacks that refer back to
their store to avoid an `Rc` cycle.

The MVP bond supports event triggers only; it has no readable output atom,
observable action calls, upstream bond chaining or error routing. Synchronous
panics propagate, and a feedback loop panics after 10,000 delivery steps.
Derived functions should be pure. Cyclic reads panic. No `unsafe` is used.

The runtime is single-threaded (`Rc` / `RefCell`). Stores retain values and graph
entries until dropped. Notifications scan active subscribers; large-graph
performance and garbage collection have not been optimized. Rust futures can be
returned by actions as ordinary values, but are neither executed nor managed.

## Validation

From the workspace root:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --target wasm32-unknown-unknown
npm ci
npx playwright install chromium
npm run test:browser
```

Playwright starts Trunk automatically unless a server is already listening on
port 8080. Browser tests exercise the actual WASM app. Node dependencies are only
for browser testing; the library and application are Rust.

## License

MIT. Independently implemented from the documented concepts. Upstream projects
are linked for attribution; this project is not an official Jotai release.

use jotai::{Atom, Store, WritableAtom};
use jotai_holo::{Action, Bond, Event, prism, state_atom};
use jotai_holo_yew::{Holo, use_holo};
use yew::prelude::*;

// #region counter-model
struct CounterModel {
    count: WritableAtom<i32>,
    doubled: Atom<i32>,
    parity: Atom<&'static str>,
    requested: Event<i32>,
    increment: Action<()>,
    route: Bond<i32>,
}
impl CounterModel {
    fn new() -> Self {
        let count = state_atom(0);
        let doubled = prism(&count, |n| n * 2);
        let parity = prism(&count, |n| if n % 2 == 0 { "even" } else { "odd" });
        let requested = Event::new();
        let increment = {
            let requested = requested.clone();
            Action::new(move |store, ()| store.emit(&requested, 1))
        };
        let route = Bond::new(&requested, |_, _| true, |_, step| *step, {
            let count = count.clone();
            Action::new(move |store, step| store.update(&count, |n| n + step))
        });
        Self {
            count,
            doubled,
            parity,
            requested,
            increment,
            route,
        }
    }
    fn binding(&self) -> Holo<CounterProps> {
        Holo::new(|| CounterProps {
            count: 0,
            on_increment: Callback::noop(),
        })
        .read(&self.count, |props, count| props.count = count)
        .write(&self.increment, |props, action| props.on_increment = action)
    }
}

// #endregion counter-model

// #region counter-view
// This View imports no model, atom, store or holo hook.
#[derive(Clone, PartialEq, Properties)]
struct CounterProps {
    count: i32,
    on_increment: Callback<()>,
}
#[component]
fn CounterView(props: &CounterProps) -> Html {
    let on_increment = props.on_increment.clone();
    html! {
        <div>
            <output class="count">{props.count}</output>
            <div><button class="increment" onclick={move |_| on_increment.emit(())}>{"Increment"}</button></div>
        </div>
    }
}

// #endregion counter-view

// #region counter-connected
#[derive(Clone, PartialEq, Properties)]
struct ConnectedProps {
    binding: Holo<CounterProps>,
    store: Store,
}
#[component]
fn ConnectedCounter(props: &ConnectedProps) -> Html {
    let view_props = use_holo(props.binding.clone(), props.store.clone());
    html! { <CounterView ..view_props /> }
}

// #endregion counter-connected

#[derive(Clone, PartialEq, Properties)]
struct DerivedProps {
    store: Store,
    doubled: Atom<i32>,
    parity: Atom<&'static str>,
}
#[derive(Clone, PartialEq)]
struct DerivedValues {
    doubled: i32,
    parity: &'static str,
}
#[component]
fn DerivedCounter(props: &DerivedProps) -> Html {
    let binding = use_memo(
        (props.doubled.clone(), props.parity.clone()),
        |(doubled, parity)| {
            Holo::new(|| DerivedValues {
                doubled: 0,
                parity: "even",
            })
            .read(doubled, |props, value| props.doubled = value)
            .read(parity, |props, value| props.parity = value)
        },
    );
    let values = use_holo((*binding).clone(), props.store.clone());
    html! { <p class="derived">{format!("Double: {} · {}", values.doubled, values.parity)}</p> }
}

#[component]
fn IndependentView() -> Html {
    let renders = use_mut_ref(|| 0);
    *renders.borrow_mut() += 1;
    html! { <small class="independent">{format!("Unrelated component renders: {}", renders.borrow())}</small> }
}

#[derive(Clone, PartialEq, Properties)]
struct PanelProps {
    title: AttrValue,
    model: RcModel,
    binding: Holo<CounterProps>,
}
#[derive(Clone)]
struct RcModel(std::rc::Rc<CounterModel>);
impl PartialEq for RcModel {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}
#[component]
fn StorePanel(props: &PanelProps) -> Html {
    let store = use_state(Store::new);
    let connected = use_state(|| true);
    let visible = use_state(|| true);
    let status = *connected;
    // #region counter-ownership
    let model = props.model.clone();
    use_effect_with((store.clone(), status), move |(store, active)| {
        let lease = active.then(|| model.0.route.connect(store));
        move || drop(lease)
    });
    // #endregion counter-ownership
    let toggle_view = {
        let visible = visible.clone();
        Callback::from(move |_| visible.set(!*visible))
    };
    let toggle_connection = {
        let connected = connected.clone();
        Callback::from(move |_| connected.set(!*connected))
    };
    let add_two = {
        let store = store.clone();
        let event = props.model.0.requested.clone();
        Callback::from(move |_| {
            store.batch(|| {
                store.emit(&event, 1);
                store.emit(&event, 1);
            })
        })
    };
    let reset_store = {
        let store = store.clone();
        Callback::from(move |_| store.set(Store::new()))
    };
    html! {
        <section>
            <h2>{&props.title}</h2>
            <ConnectedCounter binding={props.binding.clone()} store={(*store).clone()} />
            <ConnectedCounter binding={props.binding.clone()} store={(*store).clone()} />
            <button class="add-two" onclick={add_two}>{"Emit +1 twice"}</button>
            <button class="toggle-connection" onclick={toggle_connection}>
                {if status { "Disconnect bond" } else { "Connect bond" }}
            </button>
            <button class="toggle-derived" onclick={toggle_view}>{if *visible { "Hide derived" } else { "Show derived" }}</button>
            <button class="reset-store" onclick={reset_store}>{"Reset store"}</button>
            if *visible {
                <DerivedCounter store={(*store).clone()} doubled={props.model.0.doubled.clone()} parity={props.model.0.parity.clone()} />
            }
            <IndependentView />
        </section>
    }
}

#[component]
fn App() -> Html {
    let model = use_memo((), |_| RcModel(std::rc::Rc::new(CounterModel::new())));
    let binding = use_memo((), |_| model.0.binding());
    html! {
        <main>
            <h1>{"Jotai + Holo, in Rust"}</h1>
            <p>{"One model definition. Two isolated stores. Plain Yew views."}</p>
            <p>{"Each increment travels through an event and an owned bond. Both counters share their panel’s store."}</p>
            <div class="stores">
                <StorePanel title="Store A" model={(*model).clone()} binding={(*binding).clone()} />
                <StorePanel title="Store B" model={(*model).clone()} binding={(*binding).clone()} />
            </div>
        </main>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}

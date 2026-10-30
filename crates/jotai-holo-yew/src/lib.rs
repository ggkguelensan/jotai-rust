//! Typed declarative bindings. Views receive ordinary Yew props, with no atom knowledge.
use jotai::{Atom, Store, Subscription};
use jotai_holo::Action;
use std::rc::Rc;
use yew::prelude::*;

type Read<P> = dyn Fn(&Store, &mut P);
type Subscribe = dyn Fn(&Store, Rc<dyn Fn()>) -> Subscription;
type Write<P> = dyn Fn(&Store, &mut P);
struct Definition<P> {
    defaults: Rc<dyn Fn() -> P>,
    reads: Vec<Rc<Read<P>>>,
    subscriptions: Vec<Rc<Subscribe>>,
    writes: Vec<Rc<Write<P>>>,
}

/// A reusable binding definition. Build once outside render or retain with `use_memo`.
pub struct Holo<P>(Rc<Definition<P>>);
impl<P> Clone for Holo<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<P> PartialEq for Holo<P> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<P: Clone + 'static> Holo<P> {
    /// Supplies fixed props and defaults. Read/write bindings replace selected fields.
    pub fn new(defaults: impl Fn() -> P + 'static) -> Self {
        Self(Rc::new(Definition {
            defaults: Rc::new(defaults),
            reads: vec![],
            subscriptions: vec![],
            writes: vec![],
        }))
    }
    fn definition_mut(&mut self) -> &mut Definition<P> {
        Rc::get_mut(&mut self.0).expect("finish building Holo before cloning its definition")
    }
    pub fn read<T: Clone + 'static>(
        mut self,
        atom: &Atom<T>,
        assign: impl Fn(&mut P, T) + 'static,
    ) -> Self {
        let source = atom.clone();
        self.definition_mut()
            .reads
            .push(Rc::new(move |store, props| {
                assign(props, store.get(&source))
            }));
        let source = atom.clone();
        self.definition_mut()
            .subscriptions
            .push(Rc::new(move |store, notify| {
                store.subscribe(&source, move || notify())
            }));
        self
    }
    /// Creates one callback per binding/store. Action targets are never subscribed to.
    pub fn write<A: 'static>(
        mut self,
        action: &Action<A>,
        assign: impl Fn(&mut P, Callback<A>) + 'static,
    ) -> Self {
        let action = action.clone();
        self.definition_mut()
            .writes
            .push(Rc::new(move |store, props| {
                let store = store.clone();
                let action = action.clone();
                assign(props, Callback::from(move |args| action.call(&store, args)));
            }));
        self
    }
    pub fn bind(&self, store: &Store) -> Binding<P> {
        let mut props = (self.0.defaults)();
        for write in &self.0.writes {
            write(store, &mut props);
        }
        Binding(Rc::new(Bound {
            definition: self.0.clone(),
            store: store.clone(),
            props,
        }))
    }
}

struct Bound<P> {
    definition: Rc<Definition<P>>,
    store: Store,
    props: P,
}
pub struct Binding<P>(Rc<Bound<P>>);
impl<P> Clone for Binding<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<P> PartialEq for Binding<P> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<P: Clone> Binding<P> {
    pub fn snapshot(&self) -> P {
        let mut props = self.0.props.clone();
        for read in &self.0.definition.reads {
            read(&self.0.store, &mut props);
        }
        props
    }
    pub fn subscribe(&self, notify: impl Fn() + 'static) -> Vec<Subscription> {
        let notify: Rc<dyn Fn()> = Rc::new(notify);
        self.0
            .definition
            .subscriptions
            .iter()
            .map(|subscribe| subscribe(&self.0.store, notify.clone()))
            .collect()
    }
}

/// Reads bound props and releases all state subscriptions on unmount or store replacement.
#[hook]
pub fn use_holo<P: Clone + PartialEq + 'static>(holo: Holo<P>, store: Store) -> P {
    let binding = use_memo((holo, store), |(holo, store)| holo.bind(store));
    let update = use_force_update();
    let snapshot = binding.snapshot();
    let rendered = snapshot.clone();
    use_effect_with(binding.clone(), move |binding| {
        let notify = update.clone();
        let subscriptions = binding.subscribe(move || notify.force_update());
        // A child may write between render and effect. Subscribe before checking the snapshot.
        if binding.snapshot() != rendered {
            update.force_update();
        }
        move || drop(subscriptions)
    });
    snapshot
}

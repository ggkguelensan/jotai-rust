//! Framework-independent model primitives inspired by jotai-holo.
pub use jotai::{
    Atom, Event, ReadContext, Store, Subscription, WritableAtom, atom as state_atom, derived,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

type Handler<A, R> = dyn Fn(&Store, A) -> R;
/// A store-bound command. Calls batch synchronous state updates and preserve the return value.
pub struct Action<A, R = ()>(Rc<Handler<A, R>>);
impl<A, R> Clone for Action<A, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<A, R> PartialEq for Action<A, R> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<A, R> Action<A, R> {
    pub fn new(handler: impl Fn(&Store, A) -> R + 'static) -> Self {
        Self(Rc::new(handler))
    }
    pub fn call(&self, store: &Store, args: A) -> R {
        store.batch(|| (self.0)(store, args))
    }
}

pub fn event_atom<T>() -> Event<T> {
    Event::new()
}

/// A lazy projection of one atom. Compose a record or tuple with `derived` for multiple inputs.
pub fn ion<T: Clone + 'static, U: PartialEq + 'static>(
    source: &Atom<T>,
    project: impl Fn(T) -> U + 'static,
) -> Atom<U> {
    let source = source.clone();
    derived(move |get| project(get.get(&source)))
}

/// An independently cached selector; unchanged selected values do not notify subscribers.
pub fn prism<T: Clone + 'static, U: PartialEq + 'static>(
    source: &Atom<T>,
    select: impl Fn(T) -> U + 'static,
) -> Atom<U> {
    ion(source, select)
}

type Route<T> = dyn Fn(&Store, &T);
struct BondInner<T> {
    trigger: Event<T>,
    route: Rc<Route<T>>,
    connections: RefCell<HashMap<usize, Weak<Connection>>>,
}
struct Connection {
    _subscription: Subscription,
}

/// An explicitly owned event-to-action connection, shared per bond and store.
/// State can be sampled in `guard`/`map`, when the queued trigger is delivered.
pub struct Bond<T>(Rc<BondInner<T>>);
impl<T> Clone for Bond<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Multiple leases share one delivery. The final lease drop disconnects the route.
pub struct BondLease {
    _connection: Rc<Connection>,
}
impl<T: 'static> Bond<T> {
    pub fn new<A: 'static>(
        trigger: &Event<T>,
        guard: impl Fn(&Store, &T) -> bool + 'static,
        map: impl Fn(&Store, &T) -> A + 'static,
        write: Action<A>,
    ) -> Self {
        Self(Rc::new(BondInner {
            trigger: trigger.clone(),
            route: Rc::new(move |store, payload| {
                if guard(store, payload) {
                    write.call(store, map(store, payload));
                }
            }),
            connections: RefCell::default(),
        }))
    }

    pub fn connect(&self, store: &Store) -> BondLease {
        let mut connections = self.0.connections.borrow_mut();
        // Retain only live connections, including after short-lived stores disappear.
        connections.retain(|_, connection| connection.strong_count() > 0);
        let connection = connections
            .get(&store.id())
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let weak_store = store.downgrade();
                let route = self.0.route.clone();
                let subscription = store.listen(&self.0.trigger, move |payload| {
                    if let Some(store) = weak_store.upgrade() {
                        route(&store, payload);
                    }
                });
                let connection = Rc::new(Connection {
                    _subscription: subscription,
                });
                connections.insert(store.id(), Rc::downgrade(&connection));
                connection
            });
        BondLease {
            _connection: connection,
        }
    }
}

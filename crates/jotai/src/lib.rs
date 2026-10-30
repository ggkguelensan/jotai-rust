//! Single-threaded atoms with store-local values, lazy derived reads and RAII subscriptions.
use std::{
    any::Any,
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    marker::PhantomData,
    ops::Deref,
    rc::{Rc, Weak},
    sync::atomic::{AtomicUsize, Ordering},
};

type Value = Rc<dyn Any>;
type Equal = dyn Fn(&Value, &Value) -> bool;
type Compute = dyn Fn(&ReadContext) -> Value;
type EventCallback = dyn Fn(Value);
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
fn next_id() -> usize {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

struct Definition {
    id: usize,
    initial: Option<Value>,
    compute: Option<Rc<Compute>>,
    equal: Rc<Equal>,
}

/// An immutable definition. Values and computation caches belong to a [`Store`].
pub struct Atom<T> {
    definition: Rc<Definition>,
    marker: PhantomData<T>,
}
impl<T> Clone for Atom<T> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            marker: PhantomData,
        }
    }
}
impl<T> PartialEq for Atom<T> {
    fn eq(&self, other: &Self) -> bool {
        self.definition.id == other.definition.id
    }
}
impl<T> Eq for Atom<T> {}

/// A primitive atom that can be updated. Dereferences to its readable definition.
pub struct WritableAtom<T>(Atom<T>);
impl<T> Clone for WritableAtom<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> PartialEq for WritableAtom<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for WritableAtom<T> {}
impl<T> Deref for WritableAtom<T> {
    type Target = Atom<T>;
    fn deref(&self) -> &Atom<T> {
        &self.0
    }
}

fn equality<T: PartialEq + 'static>() -> Rc<Equal> {
    Rc::new(|a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>())
}

pub fn atom<T: PartialEq + 'static>(value: T) -> WritableAtom<T> {
    WritableAtom(Atom {
        definition: Rc::new(Definition {
            id: next_id(),
            initial: Some(Rc::new(value)),
            compute: None,
            equal: equality::<T>(),
        }),
        marker: PhantomData,
    })
}

pub fn derived<T: PartialEq + 'static>(read: impl Fn(&ReadContext) -> T + 'static) -> Atom<T> {
    Atom {
        definition: Rc::new(Definition {
            id: next_id(),
            initial: None,
            compute: Some(Rc::new(move |context| Rc::new(read(context)))),
            equal: equality::<T>(),
        }),
        marker: PhantomData,
    }
}

/// Reads performed here become dependencies of the current derived atom.
pub struct ReadContext {
    store: Store,
    dependencies: RefCell<HashSet<usize>>,
}
impl ReadContext {
    pub fn get<T: Clone + 'static>(&self, atom: &Atom<T>) -> T {
        (*self.get_shared(atom)).clone()
    }
    pub fn get_shared<T: 'static>(&self, atom: &Atom<T>) -> Rc<T> {
        self.dependencies.borrow_mut().insert(atom.definition.id);
        self.store.get_shared(atom)
    }
}

struct Entry {
    value: Option<Value>,
    dirty: bool,
    dependencies: HashSet<usize>,
}
struct Listener {
    atom: Rc<Definition>,
    observed: Value,
    callback: Rc<dyn Fn()>,
}
enum Delivery {
    Event(usize, Value),
}
#[derive(Default)]
struct State {
    atoms: HashMap<usize, Entry>,
    dependents: HashMap<usize, HashSet<usize>>,
    computing: HashSet<usize>,
    listeners: BTreeMap<usize, Listener>,
    events: HashMap<usize, BTreeMap<usize, Rc<EventCallback>>>,
    queue: VecDeque<Delivery>,
    batch_depth: usize,
    draining: bool,
}

#[derive(Clone)]
pub struct Store {
    id: usize,
    state: Rc<RefCell<State>>,
}
impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Store {}

/// A non-owning store reference for subscriptions that write back into their store.
#[derive(Clone)]
pub struct WeakStore {
    id: usize,
    state: Weak<RefCell<State>>,
}
impl WeakStore {
    pub fn upgrade(&self) -> Option<Store> {
        Some(Store {
            id: self.id,
            state: self.state.upgrade()?,
        })
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            id: next_id(),
            state: Rc::new(RefCell::new(State::default())),
        }
    }
    pub fn id(&self) -> usize {
        self.id
    }
    pub fn downgrade(&self) -> WeakStore {
        WeakStore {
            id: self.id,
            state: Rc::downgrade(&self.state),
        }
    }
    pub fn get<T: Clone + 'static>(&self, atom: &Atom<T>) -> T {
        (*self.get_shared(atom)).clone()
    }
    pub fn get_shared<T: 'static>(&self, atom: &Atom<T>) -> Rc<T> {
        self.read(&atom.definition)
            .downcast::<T>()
            .unwrap_or_else(|_| unreachable!("atom type mismatch"))
    }

    fn read(&self, definition: &Rc<Definition>) -> Value {
        {
            let mut state = self.state.borrow_mut();
            let entry = state.atoms.entry(definition.id).or_insert_with(|| Entry {
                value: definition.initial.clone(),
                dirty: definition.compute.is_some(),
                dependencies: HashSet::new(),
            });
            if !entry.dirty {
                return entry.value.as_ref().unwrap().clone();
            }
            assert!(
                state.computing.insert(definition.id),
                "cyclic atom dependency"
            );
        }
        let _evaluation = Evaluation {
            store: self.clone(),
            id: definition.id,
        };
        let context = ReadContext {
            store: self.clone(),
            dependencies: RefCell::default(),
        };
        let value = definition.compute.as_ref().unwrap()(&context);
        let dependencies = context.dependencies.into_inner();
        let mut state = self.state.borrow_mut();
        let old = state.atoms.get_mut(&definition.id).unwrap();
        let previous = std::mem::replace(&mut old.dependencies, dependencies.clone());
        // Preserve the cached allocation when the selected value is unchanged.
        let value = match &old.value {
            Some(previous) if (definition.equal)(previous, &value) => previous.clone(),
            _ => value,
        };
        old.value = Some(value.clone());
        old.dirty = false;
        for dependency in previous {
            if let Some(children) = state.dependents.get_mut(&dependency) {
                children.remove(&definition.id);
            }
        }
        for dependency in dependencies {
            state
                .dependents
                .entry(dependency)
                .or_default()
                .insert(definition.id);
        }
        value
    }

    pub fn set<T: PartialEq + 'static>(&self, atom: &WritableAtom<T>, value: T) {
        let previous = self.read(&atom.definition);
        let value: Value = Rc::new(value);
        if (atom.definition.equal)(&previous, &value) {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.atoms.get_mut(&atom.definition.id).unwrap().value = Some(value);
            let mut queue = VecDeque::from([atom.definition.id]);
            let mut visited = HashSet::new();
            while let Some(id) = queue.pop_front() {
                if !visited.insert(id) {
                    continue;
                }
                if let Some(children) = state.dependents.get(&id).cloned() {
                    for child in children {
                        state.atoms.get_mut(&child).unwrap().dirty = true;
                        queue.push_back(child);
                    }
                }
            }
        }
        self.drain();
    }

    pub fn update<T: Clone + PartialEq + 'static>(
        &self,
        atom: &WritableAtom<T>,
        update: impl FnOnce(T) -> T,
    ) {
        self.set(atom, update(self.get(atom)));
    }

    /// Defers notifications until the outer batch completes. Writes remain immediately readable.
    /// Batching is not a transaction: a panic does not roll back writes.
    pub fn batch<R>(&self, work: impl FnOnce() -> R) -> R {
        self.state.borrow_mut().batch_depth += 1;
        let guard = Batch(self.clone());
        let result = work();
        drop(guard);
        result
    }

    pub fn subscribe<T: 'static>(
        &self,
        atom: &Atom<T>,
        callback: impl Fn() + 'static,
    ) -> Subscription {
        let observed = self.read(&atom.definition);
        let id = next_id();
        self.state.borrow_mut().listeners.insert(
            id,
            Listener {
                atom: atom.definition.clone(),
                observed,
                callback: Rc::new(callback),
            },
        );
        Subscription {
            store: self.downgrade(),
            id,
            event: None,
        }
    }

    /// Emits every payload, including repeated equal payloads. Events retain no state.
    pub fn emit<T: 'static>(&self, event: &Event<T>, payload: T) {
        let mut state = self.state.borrow_mut();
        let listeners: Vec<_> = state
            .events
            .get(&event.id)
            .map(|items| items.keys().copied().collect())
            .unwrap_or_default();
        let value: Value = Rc::new(payload);
        for listener in listeners {
            state
                .queue
                .push_back(Delivery::Event(listener, value.clone()));
        }
        drop(state);
        self.drain();
    }

    pub fn listen<T: 'static>(
        &self,
        event: &Event<T>,
        callback: impl Fn(&T) + 'static,
    ) -> Subscription {
        let id = next_id();
        self.state
            .borrow_mut()
            .events
            .entry(event.id)
            .or_default()
            .insert(
                id,
                Rc::new(move |value| {
                    callback(value.downcast_ref().unwrap());
                }),
            );
        Subscription {
            store: self.downgrade(),
            id,
            event: Some(event.id),
        }
    }

    fn drain(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.batch_depth > 0 || state.draining {
                return;
            }
            state.draining = true;
        }
        let _guard = Drain(self.clone());
        for _ in 0..10_000 {
            let delivery = self.state.borrow_mut().queue.pop_front();
            if let Some(Delivery::Event(id, payload)) = delivery {
                let callback = self
                    .state
                    .borrow()
                    .events
                    .values()
                    .find_map(|items| items.get(&id).cloned());
                if let Some(callback) = callback {
                    callback(payload);
                }
                continue;
            }
            let definitions: Vec<_> = self
                .state
                .borrow()
                .listeners
                .iter()
                .map(|(&id, listener)| (id, listener.atom.clone()))
                .collect();
            let mut changed = Vec::new();
            for (id, definition) in definitions {
                let value = self.read(&definition);
                let mut state = self.state.borrow_mut();
                if let Some(listener) = state.listeners.get_mut(&id)
                    && !(definition.equal)(&listener.observed, &value)
                {
                    listener.observed = value;
                    changed.push((id, listener.callback.clone()));
                }
            }
            if changed.is_empty() {
                return;
            }
            for (id, callback) in changed {
                // A previous callback may have disposed this subscription.
                if self.state.borrow().listeners.contains_key(&id) {
                    callback();
                }
            }
        }
        self.state.borrow_mut().queue.clear();
        panic!("reactive feedback exceeded 10000 delivery steps");
    }
}

struct Evaluation {
    store: Store,
    id: usize,
}
impl Drop for Evaluation {
    fn drop(&mut self) {
        self.store.state.borrow_mut().computing.remove(&self.id);
    }
}
struct Batch(Store);
impl Drop for Batch {
    fn drop(&mut self) {
        self.0.state.borrow_mut().batch_depth -= 1;
        if !std::thread::panicking() {
            self.0.drain();
        }
    }
}
struct Drain(Store);
impl Drop for Drain {
    fn drop(&mut self) {
        self.0.state.borrow_mut().draining = false;
    }
}

/// Keep this guard alive to keep listening. Dropping it unsubscribes immediately.
pub struct Subscription {
    store: WeakStore,
    id: usize,
    event: Option<usize>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(store) = self.store.upgrade() {
            let mut state = store.state.borrow_mut();
            if let Some(event) = self.event {
                if let Some(items) = state.events.get_mut(&event) {
                    items.remove(&self.id);
                    if items.is_empty() {
                        state.events.remove(&event);
                    }
                }
            } else {
                state.listeners.remove(&self.id);
            }
        }
    }
}

/// A typed notification channel. Each store has its own listeners.
pub struct Event<T> {
    id: usize,
    marker: PhantomData<T>,
}
impl<T> Clone for Event<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            marker: PhantomData,
        }
    }
}
impl<T> Default for Event<T> {
    fn default() -> Self {
        Self {
            id: next_id(),
            marker: PhantomData,
        }
    }
}
impl<T> Event<T> {
    pub fn new() -> Self {
        Self::default()
    }
}

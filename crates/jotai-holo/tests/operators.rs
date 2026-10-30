use jotai_holo::{Action, Bond, Event, Store, ion, prism, state_atom};
use std::{cell::Cell, rc::Rc};

#[test]
fn action_batches_writes_and_preserves_result() {
    let store = Store::new();
    let count = state_atom(0);
    let calls = Rc::new(Cell::new(0));
    let _subscription = {
        let calls = calls.clone();
        store.subscribe(&count, move || calls.set(calls.get() + 1))
    };
    let action = {
        let count = count.clone();
        Action::new(move |store, amount| {
            store.set(&count, amount);
            store.update(&count, |n| n + 1);
            store.get(&count)
        })
    };
    assert_eq!(action.call(&store, 4), 5);
    assert_eq!(calls.get(), 1);
}

#[test]
fn selectors_are_independent_and_filter_equal_outputs() {
    let store = Store::new();
    let user = state_atom(("Ada".to_string(), 30));
    let name = prism(&user, |user| user.0);
    let age = ion(&user, |user| user.1);
    let calls = Rc::new(Cell::new(0));
    let _subscription = {
        let calls = calls.clone();
        store.subscribe(&name, move || calls.set(calls.get() + 1))
    };
    store.update(&user, |(name, age)| (name, age + 1));
    assert_eq!(store.get(&age), 31);
    assert_eq!(calls.get(), 0);
}

#[test]
fn bond_is_explicit_shared_store_local_and_disconnects_after_final_owner() {
    let a = Store::new();
    let b = Store::new();
    let count = state_atom(0);
    let trigger = Event::new();
    let bond = Bond::new(&trigger, |_, _| true, |_, value| *value, {
        let count = count.clone();
        Action::new(move |store, step| store.update(&count, |n| n + step))
    });
    a.emit(&trigger, 1);
    assert_eq!(a.get(&count), 0);
    let first = bond.connect(&a);
    let second = bond.connect(&a);
    let other = bond.connect(&b);
    a.emit(&trigger, 1);
    assert_eq!(a.get(&count), 1);
    assert_eq!(b.get(&count), 0);
    drop(first);
    a.emit(&trigger, 1);
    assert_eq!(a.get(&count), 2);
    drop(second);
    a.emit(&trigger, 1);
    assert_eq!(a.get(&count), 2);
    b.emit(&trigger, 3);
    assert_eq!(b.get(&count), 3);
    drop(other);
    let _reconnected = bond.connect(&a);
    a.emit(&trigger, 2);
    assert_eq!(a.get(&count), 4);
}

#[test]
fn guard_prevents_mapping_and_state_is_sampled_after_trigger_action_writes() {
    let store = Store::new();
    let enabled = state_atom(false);
    let value = state_atom(0);
    let output = state_atom(0);
    let trigger = Event::<()>::new();
    let maps = Rc::new(Cell::new(0));
    let bond = Bond::new(
        &trigger,
        {
            let enabled = enabled.clone();
            move |store, _| store.get(&enabled)
        },
        {
            let value = value.clone();
            let maps = maps.clone();
            move |store, _| {
                maps.set(maps.get() + 1);
                store.get(&value)
            }
        },
        {
            let output = output.clone();
            Action::new(move |store, value| store.set(&output, value))
        },
    );
    let _lease = bond.connect(&store);
    store.emit(&trigger, ());
    assert_eq!(maps.get(), 0);
    store.set(&enabled, true);
    let action = Action::new(move |store, ()| {
        store.emit(&trigger, ());
        store.set(&value, 42);
    });
    action.call(&store, ());
    assert_eq!(store.get(&output), 42);
}

#[test]
fn connection_does_not_create_store_reference_cycle() {
    let store = Store::new();
    let weak = store.downgrade();
    let event = Event::<()>::new();
    let bond = Bond::new(&event, |_, _| true, |_, _| (), Action::new(|_, ()| {}));
    let lease = bond.connect(&store);
    drop(store);
    assert!(weak.upgrade().is_none());
    drop(lease);
}

use jotai::{Store, atom};
use jotai_holo::Action;
use jotai_holo_yew::Holo;
use std::{cell::Cell, rc::Rc};
use yew::Callback;

#[derive(Clone, PartialEq)]
struct Props {
    title: &'static str,
    count: i32,
    increment: Callback<()>,
}

#[test]
fn ordinary_props_stable_actions_and_subscription_cleanup() {
    let count = atom(0);
    let action = {
        let count = count.clone();
        Action::new(move |store, ()| store.update(&count, |n| n + 1))
    };
    let holo = Holo::new(|| Props {
        title: "Counter",
        count: 0,
        increment: Callback::noop(),
    })
    .read(&count, |props, value| props.count = value)
    .write(&action, |props, callback| props.increment = callback);
    let a = Store::new();
    let b = Store::new();
    let binding = holo.bind(&a);
    let other = holo.bind(&b);
    let before = binding.snapshot();
    let notifications = Rc::new(Cell::new(0));
    let subscriptions = {
        let notifications = notifications.clone();
        binding.subscribe(move || notifications.set(notifications.get() + 1))
    };
    before.increment.emit(());
    let after = binding.snapshot();
    assert_eq!(after.title, "Counter");
    assert_eq!(after.count, 1);
    assert_eq!(after.increment, before.increment);
    assert_eq!(other.snapshot().count, 0);
    assert_eq!(notifications.get(), 1);
    drop(subscriptions);
    after.increment.emit(());
    assert_eq!(notifications.get(), 1);
    assert_eq!(binding.snapshot().count, 2);
}

#[test]
fn action_only_binding_does_not_subscribe_to_state() {
    let count = atom(0);
    let action = {
        let count = count.clone();
        Action::new(move |store, ()| store.update(&count, |n| n + 1))
    };
    let binding = Holo::new(|| Props {
        title: "Action only",
        count: 0,
        increment: Callback::noop(),
    })
    .write(&action, |props, callback| props.increment = callback)
    .bind(&Store::new());
    let subscriptions = binding.subscribe(|| panic!("action bindings must not subscribe"));
    assert!(subscriptions.is_empty());
    binding.snapshot().increment.emit(());
}

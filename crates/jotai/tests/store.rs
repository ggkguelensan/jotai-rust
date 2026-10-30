use jotai::{Event, Store, atom, derived};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn stores_isolate_values_and_share_definitions() {
    let count = atom(1);
    let doubled = {
        let count = count.clone();
        derived(move |get| get.get(&count) * 2)
    };
    let a = Store::new();
    let b = Store::new();
    a.set(&count, 7);
    assert_eq!(a.get(&doubled), 14);
    assert_eq!(b.get(&doubled), 2);
}

#[test]
fn derived_reads_are_lazy_cached_and_stop_work_after_unsubscribe() {
    let store = Store::new();
    let count = atom(0);
    let calls = Rc::new(Cell::new(0));
    let selected = {
        let calls = calls.clone();
        let count = count.clone();
        derived(move |get| {
            calls.set(calls.get() + 1);
            get.get(&count) * 2
        })
    };
    store.set(&count, 1);
    assert_eq!(calls.get(), 0);
    let subscription = store.subscribe(&selected, || {});
    assert_eq!(store.get(&selected), 2);
    assert_eq!(calls.get(), 1);
    store.set(&count, 2);
    assert_eq!(calls.get(), 2);
    drop(subscription);
    store.set(&count, 3);
    assert_eq!(calls.get(), 2);
    assert_eq!(store.get(&selected), 6);
    assert_eq!(calls.get(), 3);
}

#[test]
fn dynamic_dependencies_remove_inactive_branches() {
    let store = Store::new();
    let choose_a = atom(true);
    let a = atom(1);
    let b = atom(10);
    let calls = Rc::new(Cell::new(0));
    let selected = {
        let (choose_a, a, b, calls) = (choose_a.clone(), a.clone(), b.clone(), calls.clone());
        derived(move |get| {
            calls.set(calls.get() + 1);
            if get.get(&choose_a) {
                get.get(&a)
            } else {
                get.get(&b)
            }
        })
    };
    let _subscription = store.subscribe(&selected, || {});
    store.set(&b, 20);
    assert_eq!(calls.get(), 1);
    store.set(&choose_a, false);
    assert_eq!(store.get(&selected), 20);
    store.set(&a, 2);
    assert_eq!(calls.get(), 2);
}

#[test]
fn unchanged_projection_does_not_notify_or_replace_shared_value() {
    let store = Store::new();
    let count = atom(0);
    let parity = {
        let count = count.clone();
        derived(move |get| get.get(&count) % 2)
    };
    let notifications = Rc::new(Cell::new(0));
    let _subscription = {
        let n = notifications.clone();
        store.subscribe(&parity, move || n.set(n.get() + 1))
    };
    let before = store.get_shared(&parity);
    store.set(&count, 2);
    assert_eq!(notifications.get(), 0);
    assert!(Rc::ptr_eq(&before, &store.get_shared(&parity)));
    store.set(&count, 3);
    assert_eq!(notifications.get(), 1);
}

#[test]
fn diamond_and_nested_batches_expose_only_consistent_final_values() {
    let store = Store::new();
    let count = atom(1);
    let left = {
        let count = count.clone();
        derived(move |get| get.get(&count) + 1)
    };
    let right = {
        let count = count.clone();
        derived(move |get| get.get(&count) * 2)
    };
    let total = derived(move |get| get.get(&left) + get.get(&right));
    let seen = Rc::new(RefCell::new(vec![]));
    let _subscription = {
        let (store_ref, total, seen) = (store.downgrade(), total.clone(), seen.clone());
        store.subscribe(&total.clone(), move || {
            seen.borrow_mut()
                .push(store_ref.upgrade().unwrap().get(&total))
        })
    };
    store.batch(|| {
        store.set(&count, 2);
        store.batch(|| store.set(&count, 3));
    });
    assert_eq!(*seen.borrow(), [10]);
    store.batch(|| {
        store.set(&count, 4);
        store.set(&count, 3);
    });
    assert_eq!(*seen.borrow(), [10]);
}

#[test]
fn subscriber_can_write_and_dispose_another_subscriber() {
    let store = Store::new();
    let count = atom(0);
    let output = atom(0);
    let second = Rc::new(RefCell::new(None));
    let _first = {
        let (weak, count, output, second) = (
            store.downgrade(),
            count.clone(),
            output.clone(),
            second.clone(),
        );
        store.subscribe(&count.clone(), move || {
            second.borrow_mut().take();
            let store = weak.upgrade().unwrap();
            store.set(&output, store.get(&count) * 2);
        })
    };
    *second.borrow_mut() =
        Some(store.subscribe(&count, || panic!("disposed listener must not run")));
    store.set(&count, 3);
    assert_eq!(store.get(&output), 6);
}

#[test]
fn repeated_events_are_fifo_and_disposed_queued_deliveries_are_skipped() {
    let store = Store::new();
    let event = Event::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let subscription = {
        let seen = seen.clone();
        store.listen(&event, move |value| seen.borrow_mut().push(*value))
    };
    store.batch(|| {
        store.emit(&event, 1);
        store.emit(&event, 1);
        store.emit(&event, 2);
    });
    assert_eq!(*seen.borrow(), [1, 1, 2]);
    store.batch(|| {
        store.emit(&event, 3);
        drop(subscription);
    });
    assert_eq!(*seen.borrow(), [1, 1, 2]);
    let other = Store::new();
    other.emit(&event, 4);
    assert_eq!(*seen.borrow(), [1, 1, 2]);
}

#[test]
fn panic_does_not_leave_batch_or_derived_evaluation_locked() {
    let store = Store::new();
    let source = atom(0);
    let failing = {
        let source = source.clone();
        derived(move |get| {
            let n = get.get(&source);
            assert_ne!(n, 0);
            n
        })
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| store.get(&failing))).is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| store.batch(|| {
            store.set(&source, 1);
            panic!("abort");
        })))
        .is_err()
    );
    assert_eq!(store.get(&failing), 1);
    store.set(&source, 2);
    assert_eq!(store.get(&failing), 2);
}

#[test]
fn subscription_does_not_retain_store() {
    let store = Store::new();
    let weak = store.downgrade();
    let subscription = store.subscribe(&atom(1), || {});
    drop(store);
    assert!(weak.upgrade().is_none());
    drop(subscription);
}

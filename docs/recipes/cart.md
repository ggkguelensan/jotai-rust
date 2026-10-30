# Корзина: несколько источников и проекции

Количество и цена — независимые атомы. Итог вычисляется из обоих источников,
а View может подписаться только на нужные ему значения.

В примере цены представлены целыми копейками/центами, без floating point.
Ограничения диапазона здесь учебные: для production-расчётов добавьте проверку
переполнения и правила валюты.

```rust
use jotai_holo::{Action, Atom, Store, WritableAtom, derived, ion, state_atom};

struct Cart {
    quantity: WritableAtom<u32>,
    price: WritableAtom<u32>,
    total: Atom<u32>,
    label: Atom<String>,
    set_quantity: Action<u32>,
}

fn create_cart() -> Cart {
    let quantity = state_atom(1_u32);
    let price = state_atom(1250_u32);
    let total = {
        let (quantity, price) = (quantity.clone(), price.clone());
        derived(move |get| get.get(&quantity) * get.get(&price))
    };
    let label = ion(&total, |cents| format!("{}.{:02}", cents / 100, cents % 100));
    let set_quantity = {
        let quantity = quantity.clone();
        Action::new(move |store, value: u32| store.set(&quantity, value.min(99)))
    };
    Cart { quantity, price, total, label, set_quantity }
}

let cart = create_cart();
let store = Store::new();
cart.set_quantity.call(&store, 3);
assert_eq!(store.get(&cart.quantity), 3);
assert_eq!(store.get(&cart.total), 3750);
assert_eq!(store.get(&cart.label), "37.50");

store.batch(|| {
    store.set(&cart.price, 1000);
    cart.set_quantity.call(&store, 2);
});
assert_eq!(store.get(&cart.label), "20.00");
```

## Какие части отдать View

Для строки количества используйте `.read(&cart.quantity, ...)` и
`.write(&cart.set_quantity, ...)`. Для итоговой суммы достаточно
`.read(&cart.label, ...)`. Так изменение цены не обязано перерисовывать компонент,
который наблюдает только количество.

В отличие от TypeScript `ion({ quantity, price }, project)`, Rust `ion` сейчас
принимает один источник. `derived` явно объединяет несколько атомов; каждая
операция `get` регистрирует зависимость.

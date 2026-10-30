# ion, prism и Bond

`ion` и `prism` создают читаемые атомы. `Bond` соединяет события с действиями
под явным владением. Все они работают без Yew.

Выбирайте `ion` для преобразования значения: удвоение числа, форматирование,
вычисление признака. `prism` используйте для выделения отдельной проекции
составного состояния, например имени из профиля. В текущем MVP механика у них
одинаковая; различие выражает намерение кода.

## `ion(&source, project)`

Создаёт ленивый read-only `Atom<U>` из одного `Atom<T>`:
`T: Clone + 'static`, `U: PartialEq + 'static`.
Зависимости, кеш и store isolation обеспечивает ядро.

```rust
use jotai_holo::{Store, ion, state_atom};

let amount = state_atom(1250_u32);
let label = ion(&amount, |cents| format!("{}.{:02}", cents / 100, cents % 100));
assert_eq!(Store::new().get(&label), "12.50");
```

Для нескольких источников используйте `derived`, как в
[рецепте корзины](../recipes/cart.md). Объектной/tuple перегрузки `ion` пока нет.

## `prism(&source, select)`

В текущей реализации вызывает `ion` с тем же контрактом. Название выражает
намерение выделить независимую проекцию. Несколько полей — несколько вызовов.

```rust
use jotai_holo::{Store, prism, state_atom};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, PartialEq)]
struct User { name: String, age: u8 }

let user = state_atom(User { name: "Ada".into(), age: 30 });
let name = prism(&user, |user| user.name);
let adult = prism(&user, |user| user.age >= 18);
let store = Store::new();
let calls = Rc::new(Cell::new(0));
let _subscription = {
    let calls = calls.clone();
    store.subscribe(&name, move || calls.set(calls.get() + 1))
};
store.update(&user, |mut user| { user.age += 1; user });
assert_eq!(store.get(&name), "Ada");
assert!(store.get(&adult));
assert_eq!(calls.get(), 0);
```

Селектор имени может вычислиться заново, но равный `String` не вызовет уведомление.
Неиспользуемый sibling-селектор остаётся ленивым. Deep field tracking и writable
lenses отсутствуют.

## `Bond::new(&trigger, guard, map, write)`

| Аргумент | Тип / назначение |
| --- | --- |
| `trigger` | `&Event<T>` |
| `guard` | `Fn(&Store, &T) -> bool`; false пропускает map и действие |
| `map` | `Fn(&Store, &T) -> A`; можно прочитать текущее состояние |
| `write` | `Action<A, ()>` |

Все аргументы обязательны. Чтобы пропускать все события, передайте
`|_, _| true`. Read внутри guard/map не создаёт отдельную подписку на состояние.

```rust
use jotai_holo::{Action, Bond, Event, Store, state_atom};

let count = state_atom(0);
let requested = Event::<i32>::new();
let route = Bond::new(
    &requested,
    |_, amount| *amount > 0,
    |_, amount| *amount,
    {
        let count = count.clone();
        Action::new(move |store, amount| store.update(&count, |n| n + amount))
    },
);
let store = Store::new();
store.emit(&requested, 5);
assert_eq!(store.get(&count), 0); // Нет активного владельца.

let first = route.connect(&store);
let second = route.connect(&store);
store.emit(&requested, 2);
assert_eq!(store.get(&count), 2); // Два владельца, одна доставка.
store.emit(&requested, -1);
assert_eq!(store.get(&count), 2);

drop(first);
store.emit(&requested, 1);
assert_eq!(store.get(&count), 3);
drop(second);
store.emit(&requested, 1);
assert_eq!(store.get(&count), 3);
```

## Владение и доставка

`connect` возвращает `BondLease`. Соединение разделяется только между владельцами
той же идентичности Bond и того же Store; `Bond::clone()` сохраняет идентичность.
Создание нового Bond с похожими замыканиями создаёт другое соединение.

Последний `Drop` отключает маршрут. Повторный `connect` подключает его снова,
не сбрасывая значения атомов. Само соединение не удерживает Store сильной ссылкой.

События обрабатываются FIFO. Более раннее действие может изменить состояние,
которое прочитает следующий map. Это не transaction-wide snapshot и не граф
приоритетов Effector. Если важны исходные данные, передайте их в payload.
Бесконечная обратная связь останавливается panic после 10 000 шагов доставки.

Нет output-атома Bond, chaining, readable triggers, action triggers и async
error routing. Для живого вычисляемого состояния используйте `ion` / `prism`.

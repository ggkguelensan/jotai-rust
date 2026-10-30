# Holo, Binding и use_holo

`jotai-holo-yew` связывает модель с обычной структурой props. Он не создаёт
новый тип компонента: полученные props вы передаёте выбранному Yew View.

## `Holo::new(defaults)`

Создаёт определение `Holo<P>`, где `P: Clone + 'static`.
Фабрика defaults возвращает полную структуру props. Она вызывается при `bind`,
а не при создании определения. Здесь удобно задать заголовки и начальные callbacks.

## `.read(&atom, assign)`

При каждом snapshot читает значение атома и вызывает `assign(&mut P, T)`.
Значение требует `Clone`. Каждая read-привязка добавляет подписку, когда
`Binding::subscribe` или `use_holo` активируют наблюдение.

## `.write(&action, assign)`

Создаёт `yew::Callback<A>` из `Action<A, ()>` и передаёт его в
`assign(&mut P, Callback<A>)`. Callback привязан к выбранному Store и сохраняет
идентичность между snapshot одной Binding. На состояние цели подписки нет.

```rust
use jotai::{Store, atom};
use jotai_holo::Action;
use jotai_holo_yew::Holo;
use yew::Callback;

#[derive(Clone)]
struct Props {
    title: &'static str,
    count: i32,
    on_add: Callback<i32>,
}

let count = atom(0);
let add = {
    let count = count.clone();
    Action::new(move |store, amount| store.update(&count, |n| n + amount))
};
let holo = Holo::new(|| Props {
    title: "Счётчик",
    count: 0,
    on_add: Callback::noop(),
})
.read(&count, |props, value| props.count = value)
.write(&add, |props, callback| props.on_add = callback);

let store = Store::new();
let binding = holo.bind(&store);
let before = binding.snapshot();
before.on_add.emit(3);
let after = binding.snapshot();
assert_eq!(after.title, "Счётчик");
assert_eq!(after.count, 3);
assert_eq!(before.on_add, after.on_add);
```

Сначала вызываются defaults, затем write-привязки при bind, а read-привязки —
при snapshot. Если замыкания меняют одно поле, последнее присваивание выигрывает.
Проверки конфликтов полей нет. Завершите builder до клонирования Holo:
добавление read/write после клонирования общего определения вызывает panic.

## `Holo::bind(&store) -> Binding<P>`

Создаёт привязку и её стабильные callbacks, но не начинает наблюдение.
Новый `bind` создаёт новую Binding и новые callbacks, даже для того же Store.
`Binding::clone()` сохраняет существующие.

| Метод Binding | Поведение |
| --- | --- |
| `snapshot() -> P` | Клонирует базовые props и накладывает актуальные read-значения |
| `subscribe(notify) -> Vec<Subscription>` | Создаёт подписки на все read-источники; сохраните весь Vec |

Владение Binding и callbacks удерживает Store. Возвращённые Subscription
сами по себе содержат слабую ссылку на Store.

## `use_holo(holo, store) -> P`

Yew hook требует `P: Clone + PartialEq + 'static`.
Он мемоизирует Binding по идентичности Holo и Store, читает snapshot при рендере
и подписывается в effect. После подключения сверяет snapshot, чтобы не пропустить
изменение между render и effect.

На unmount или замену определения/Store старые подписки освобождаются.
При замене Store callbacks новой Binding обращаются к новому хранилищу.

Схема соединяющего компонента и обычного View взята из работающего примера:

<<< @/../examples/yew-counter/src/main.rs#counter-view

<<< @/../examples/yew-counter/src/main.rs#counter-connected

`CounterView` можно напрямую отрисовать с обычными props, без Holo.
Для истории вызовов используйте обычный `Callback`, собирающий значения в тесте.

## Что не делает адаптер

Он не включает Bond, не создаёт registry моделей, не вычисляет недостающие public
props и не передаёт входные props в атомы. Fixed props задаются фабрикой defaults;
изменяемые props родителя передавайте явно по правилам Yew.

Определение Holo держите в `use_memo` или в долгоживущей модели. Создавать его
заново на каждом рендере означает создавать новые Binding и подписки.

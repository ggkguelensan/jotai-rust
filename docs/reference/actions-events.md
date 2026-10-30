# Action и Event

Действие выполняет команду. Событие сообщает о факте без сохранения текущего
значения. Они отделены от читаемых атомов.

## `Action::new(handler)`

`Action<A, R = ()>` хранит `Fn(&Store, A) -> R + 'static`.
`call(&store, args)` выполняет handler внутри `Store::batch` и возвращает его
результат. `clone()` сохраняет идентичность действия, но не привязывает его к Store.

```rust
use jotai_holo::{Action, Store, state_atom};

let count = state_atom(0);
let add = {
    let count = count.clone();
    Action::new(move |store, amount: i32| {
        store.update(&count, |n| n + amount);
        store.get(&count)
    })
};
let store = Store::new();
assert_eq!(add.call(&store, 3), 3);
assert_eq!(add.call(&store, 2), 5);
```

Нет аргумента — используйте `()`. Несколько аргументов — tuple или структуру.
Для ожидаемой ошибки возвращайте обычный `Result`:

```rust
use jotai_holo::{Action, Store, state_atom};

let age = state_atom(0_u8);
let set_age = {
    let age = age.clone();
    Action::new(move |store, value: i32| -> Result<(), &'static str> {
        let valid = u8::try_from(value).map_err(|_| "Возраст вне диапазона")?;
        store.set(&age, valid);
        Ok(())
    })
};
let store = Store::new();
assert_eq!(set_age.call(&store, -1), Err("Возраст вне диапазона"));
assert_eq!(store.get(&age), 0);
```

`Result::Err` не откатывает предыдущие записи handler.
Yew `Holo::write` и цель `Bond` пока принимают только `Action<A, ()>`.
Сделайте отдельное действие, которое обрабатывает Result и записывает статус,
если нужно связать такую команду с UI.

## `Event<T>` / `event_atom::<T>()`

Событие — типизированный канал. Значения и начального snapshot у него нет.
Подписчики принадлежат конкретному Store.

```rust
use jotai_holo::{Store, event_atom};
use std::{cell::RefCell, rc::Rc};

let store = Store::new();
let submitted = event_atom::<String>();
let received = Rc::new(RefCell::new(Vec::new()));
let listener = {
    let received = received.clone();
    store.listen(&submitted, move |name| received.borrow_mut().push(name.clone()))
};
store.batch(|| {
    store.emit(&submitted, "Ada".into());
    store.emit(&submitted, "Ada".into());
});
assert_eq!(*received.borrow(), ["Ada", "Ada"]);
drop(listener);
```

Повторные равные payload доставляются отдельно и в порядке FIFO очереди.
Подписка после публикации не получает историю. Если подписка уничтожена до
обработки её queued delivery, доставка пропускается.

## Порядок и ошибки

Без внешнего batch доставка завершается до возврата `emit`. Внутри `Action`
или batch события ждут завершения синхронных записей. Bond читает guard и state
при обработке, поэтому они могут отличаться от значений на момент `emit`.

Вызов Action сам по себе не публикует событие. Если нужен факт успешного
выполнения, явно вызовите `emit` после успешной работы. Panic не превращается
в error-событие; runtime не перехватывает ошибки пользователя.
Future в результате действия остаётся обычным future: executor и отмену
нужно организовать отдельно.

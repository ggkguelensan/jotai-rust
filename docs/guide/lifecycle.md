# Владение и подписки

Создание атома, Holo или Bond описывает работу. Подписка и `connect` определяют,
кто и как долго владеет активным ресурсом.

| Ресурс | Что запускает | Что завершает |
| --- | --- | --- |
| Вычисляемый атом | Первый read; обновления зависимостей при активной подписке | После последней отписки вычисления ждут следующего read |
| `Subscription` | `Store::subscribe` или `Store::listen` | `Drop` guard |
| `BondLease` | `Bond::connect` | `Drop` последнего lease для этой пары Bond/Store |
| `Binding` | `Holo::bind` создаёт defaults и callbacks | Сам по себе не подписывается |
| `use_holo` | Подписка на `read` в Yew effect | Unmount или замена определения/Store |

## Сохраните guard

Если сразу уничтожить возвращённую подписку, callback больше не будет вызван.
Имя `_subscription` сохраняет guard до конца области видимости; `let _ = ...`
его не сохраняет.

```rust
use jotai::{Store, atom};
use std::{cell::Cell, rc::Rc};

let store = Store::new();
let count = atom(0);
let calls = Rc::new(Cell::new(0));
let subscription = {
    let calls = calls.clone();
    store.subscribe(&count, move || calls.set(calls.get() + 1))
};
store.set(&count, 1);
assert_eq!(calls.get(), 1);
drop(subscription);
store.set(&count, 2);
assert_eq!(calls.get(), 1);
```

## Обратная ссылка на Store

Store владеет callbacks активных подписок. Захват сильной копии того же Store
в callback может создать цикл `Rc`. Используйте `downgrade` и `upgrade`:

```rust
use jotai::{Store, atom};

let store = Store::new();
let source = atom(0);
let destination = atom(0);
let weak = store.downgrade();
let subscription = {
    let source_for_callback = source.clone();
    let destination = destination.clone();
    store.subscribe(&source, move || {
        if let Some(store) = weak.upgrade() {
            store.set(&destination, store.get(&source_for_callback) * 2);
        }
    })
};
store.set(&source, 3);
assert_eq!(store.get(&destination), 6);
drop(subscription);
```

Для чистых вычисляемых значений предпочтительнее `derived`, а не запись из
подписки. Этот пример показывает безопасную обратную ссылку для интеграций.

## Соединение в Yew

`use_holo` не активирует Bond автоматически. Владелец модели отдельно подключает
его в effect и возвращает очистку, которая уничтожает lease. В рабочем
[примере](../examples/counter.md) это делает `StorePanel`.

Чтение и рендер не должны запускать соединения или внешние побочные эффекты.
У Rust-адаптера нет контракта React StrictMode или Suspense: здесь действуют
жизненный цикл Yew и явное владение guard.

## Что остаётся после отключения

Значения атомов и кешированные записи сохраняются до уничтожения Store.
Отписка останавливает наблюдение, но не очищает данные и не отменяет уже начатую
внешнюю работу. Автоматических GC, persistence и async cancellation пока нет.

# Store и Subscription

Каждый `Store::new()` создаёт изолированные значения, кеш вычислений, очередь
событий и подписки. `clone()` создаёт handle того же хранилища.

## Методы

| Метод | Контракт |
| --- | --- |
| `get(&atom) -> T` | Читает и клонирует значение; требует `T: Clone + 'static` |
| `get_shared(&atom) -> Rc<T>` | Читает разделяемое значение без `Clone` у `T` |
| `set(&writable, value)` | Записывает значение; равное по `PartialEq` игнорируется |
| `update(&writable, fn)` | Читает `T`, вычисляет новое значение и вызывает `set` |
| `batch(fn) -> R` | Откладывает доставку событий и уведомления до завершения внешнего batch |
| `subscribe(&atom, fn) -> Subscription` | Наблюдает изменения значения; не вызывает callback сразу |
| `emit(&event, payload)` | Публикует типизированное событие |
| `listen(&event, fn) -> Subscription` | Слушает события; callback получает `&T` |
| `downgrade() -> WeakStore` | Возвращает слабый handle; `upgrade` может вернуть `None` |
| `id() -> usize` | Идентификатор runtime для этой пары handle/Store; не persistence key |

## Подписка на значение

`subscribe` читает исходное значение, чтобы получить начальный snapshot.
Callback получает `()`: актуальное значение можно прочитать из слабого Store
или получить в следующем рендере. Храните возвращённый guard.

```rust
use jotai::{Store, atom};
use std::{cell::RefCell, rc::Rc};

let store = Store::new();
let count = atom(0);
let history = Rc::new(RefCell::new(Vec::new()));
let subscription = {
    let history = history.clone();
    let weak = store.downgrade();
    let source = count.clone();
    store.subscribe(&count, move || {
        if let Some(store) = weak.upgrade() {
            history.borrow_mut().push(store.get(&source));
        }
    })
};
store.set(&count, 1);
store.set(&count, 1);
assert_eq!(*history.borrow(), [1]);
drop(subscription);
```

## Batch

Записи сразу доступны для чтения, но callbacks ждут внешней границы batch.
Изменения `0 → 1 → 0` внутри одного batch не уведомляют подписчика состояния.
События при этом не отсекаются по равенству.

```rust
use jotai::{Store, atom, derived};
use std::{cell::Cell, rc::Rc};

let store = Store::new();
let first = atom(1);
let second = atom(2);
let sum = {
    let (first, second) = (first.clone(), second.clone());
    derived(move |get| get.get(&first) + get.get(&second))
};
let calls = Rc::new(Cell::new(0));
let _subscription = {
    let calls = calls.clone();
    store.subscribe(&sum, move || calls.set(calls.get() + 1))
};
let result = store.batch(|| {
    store.set(&first, 10);
    store.batch(|| store.set(&second, 20));
    store.get(&sum)
});
assert_eq!(result, 30);
assert_eq!(calls.get(), 1);
```

Batch не обеспечивает rollback. Если код паникует, записи остаются; доставка
не выполняется во время раскрутки стека. Порядок обработки изменённых подписок
не следует использовать как механизм бизнес-логики: координируйте изменения
в одном `Action` или передавайте необходимые данные в payload события.

## Ограничения

Store однопоточный и использует `Rc` / `RefCell`. Graph-записи и значения
сохраняются до его уничтожения; отдельного удаления атома нет.
Доставка изменений просматривает активные подписки. API пока не рассчитан
на передачу между потоками, сериализацию Store или большие графы без измерений.

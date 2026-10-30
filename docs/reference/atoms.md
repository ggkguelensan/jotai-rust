# atom, derived и ReadContext

## `atom(value)` / `state_atom(value)`

Создаёт `WritableAtom<T>`. Тип значения должен реализовать `PartialEq` и не
содержать заимствований с ограниченным временем жизни (`'static`). Сам тип может
владеть `String`, `Vec` или `Rc`. Требования `Send` / `Sync` нет.

`state_atom` из `jotai_holo` — alias примитивного `jotai::atom`.

```rust
use jotai::{Store, atom};

let title = atom(String::from("Черновик"));
let store = Store::new();
store.set(&title, String::from("Готово"));
assert_eq!(store.get(&title), "Готово");
```

`WritableAtom<T>` разыменовывается в `Atom<T>`, поэтому его можно передавать в
read API. Создать независимое определение можно новым вызовом `atom`;
`clone()` сохраняет идентичность существующего атома.

## `derived(read)`

Создаёт read-only `Atom<T>`, где `T: PartialEq + 'static`.
Замыкание имеет тип `Fn(&ReadContext) -> T + 'static`.
Выполняйте внутри только чистые вычисления; изменения состояния и события
оставляйте действиям.

```rust
use jotai::{Store, atom, derived};

let price = atom(100_u32);
let quantity = atom(2_u32);
let total = {
    let (price, quantity) = (price.clone(), quantity.clone());
    derived(move |get| get.get(&price) * get.get(&quantity))
};
let store = Store::new();
assert_eq!(store.get(&total), 200);
store.set(&quantity, 3);
assert_eq!(store.get(&total), 300);
```

Определение не вычисляется при создании. Первый read выполняет замыкание и
запоминает зависимости. Повторные reads используют кеш, пока зависимость не
изменится. При активной подписке runtime пересчитывает затронутый атом, чтобы
определить, нужно ли уведомление. Равный результат сохраняет прежний `Rc` кеша.

## Условные зависимости

Зависимостями становятся только фактически прочитанные атомы. После повторного
вычисления runtime удаляет связи, которые больше не использовались.

```rust
use jotai::{Store, atom, derived};

let preview = atom(true);
let draft = atom(String::from("Черновик"));
let published = atom(String::from("Публикация"));
let displayed = {
    let (preview, draft, published) = (preview.clone(), draft.clone(), published.clone());
    derived(move |get| {
        if get.get(&preview) { get.get(&draft) } else { get.get(&published) }
    })
};
let store = Store::new();
assert_eq!(store.get(&displayed), "Черновик");
store.set(&preview, false);
assert_eq!(store.get(&displayed), "Публикация");
```

## `ReadContext::get` / `get_shared`

Оба метода регистрируют зависимость. `get` требует `T: Clone` и возвращает копию
значения. `get_shared` возвращает `Rc<T>` без требования `Clone` к `T`.

```rust
use jotai::{Store, atom, derived};

#[derive(PartialEq)]
struct Catalog { items: Vec<String> }

let catalog = atom(Catalog { items: vec!["Rust".into()] });
let length = derived(move |get| get.get_shared(&catalog).items.len());
assert_eq!(Store::new().get(&length), 1);
```

Нельзя вызвать `Store::set` для `Atom<T>` без writable handle. Попытка сделать
циклическое чтение вызывает panic. Writable derivations и async reads пока не
поддерживаются.

# Профиль: событие, guard и действие

Редактор хранит черновик. Событие сообщает о запросе сохранения, Bond проверяет
допустимость и читает актуальный черновик, действие записывает подтверждённое
значение. Этот пример синхронный: «сохранение» означает запись в другой атом,
без HTTP или persistence.

```text
draft → can_save ────────────┐
                            ↓ guard
save_requested → Bond → snapshot draft → save → saved
                  ↑
            владелец BondLease
```

```rust
use jotai_holo::{
    Action, Atom, Bond, Event, Store, WritableAtom, prism, state_atom,
};

#[derive(Clone, PartialEq, Debug)]
struct Profile { name: String }

struct Editor {
    draft: WritableAtom<Profile>,
    saved: WritableAtom<Option<Profile>>,
    can_save: Atom<bool>,
    save_requested: Event<()>,
    saving: Bond<()>,
}

fn create_editor() -> Editor {
    let draft = state_atom(Profile { name: String::new() });
    let saved = state_atom(None::<Profile>);
    let can_save = prism(&draft, |profile| !profile.name.trim().is_empty());
    let save_requested = Event::new();
    let save = {
        let saved = saved.clone();
        Action::new(move |store, profile| store.set(&saved, Some(profile)))
    };
    let saving = Bond::new(
        &save_requested,
        { let can_save = can_save.clone(); move |store, _| store.get(&can_save) },
        { let draft = draft.clone(); move |store, _| store.get(&draft) },
        save,
    );
    Editor { draft, saved, can_save, save_requested, saving }
}

let editor = create_editor();
let store = Store::new();
let lease = editor.saving.connect(&store);

store.emit(&editor.save_requested, ());
assert!(!store.get(&editor.can_save));
assert_eq!(store.get(&editor.saved), None);

store.batch(|| {
    store.emit(&editor.save_requested, ());
    store.set(&editor.draft, Profile { name: "Ada".into() });
});
assert_eq!(store.get(&editor.saved), Some(Profile { name: "Ada".into() }));

drop(lease);
store.set(&editor.draft, Profile { name: "Grace".into() });
store.emit(&editor.save_requested, ());
assert_eq!(store.get(&editor.saved), Some(Profile { name: "Ada".into() }));
```

## Почему сохраняется Ada

Во втором batch событие отправлено до записи черновика, но обработано после
завершения batch. Guard и map читают состояние на момент доставки. Если нужно
сохранить именно значение на момент клика, включите `Profile` в payload Event.

## Подключение кнопки

`Holo::write` принимает Action, поэтому для кнопки создайте действие
`Action::new(move |store, ()| store.emit(&event, ()))`, захватив clone события.
View получит обычный `Callback<()>`.

Владелец редактора хранит lease или создаёт его в Yew effect. Простое чтение
`draft` и рендер View не подключают `saving`. Для нескольких владельцев используйте
один Bond: повторный connect к тому же Store не удвоит сохранения.

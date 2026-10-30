# Переход с TypeScript jotai-holo

Rust-версия сохраняет разделение состояния, модели и View. Она пока реализует
часть возможностей [исходного проекта](https://github.com/ggkguelensan/jotai-holo).
Эта таблица описывает текущий код, а не обещание совместимости.

| TypeScript | Rust | Отличие |
| --- | --- | --- |
| `stateAtom(value)` | `state_atom(value)` / `atom(value)` | `T: PartialEq + 'static` |
| `stateAtom(get => ...)` | `derived(...)` | Отдельная функция; нет writable derived atoms |
| `actionAtom(...)` | `Action::new(...)` | `call(&store, args)`; несколько аргументов передаются tuple/struct |
| `eventAtom<T>()` | `event_atom::<T>()` / `Event::<T>::new()` | Доставка через `store.emit`, наблюдение через `store.listen` |
| `ion(record/tuple, project)` | `derived(...)` для нескольких источников | `ion` сейчас принимает один атом |
| `prism(source, selectors)` | Несколько вызовов `prism(&source, select)` | Каждый вызов создаёт отдельный атом; writable lenses нет |
| `bond({ ... })` | `Bond::new(&event, guard, map, action)` | Только Event-trigger, все аргументы обязательны |
| `holo({ view, read, write, static })` | `Holo::new(defaults).read(...).write(...)` | View рендерится отдельно с props из `use_holo` |
| `static` | Поля в `Holo::new` | Defaults вычисляются при `bind` |
| `Provider` | Явная передача `Store` | Встроенного default Store или provider нет |
| Registry / model lifecycle | Обычная фабрика + `Rc` + guards | Автоматический выбор по ключу, retention, leases модели не реализованы |
| `input`, `holoList`, `holoMatch` | Пока нет | Используйте обычные props, списки и ветвления Yew |
| Query, RHF, analytics, persistence | Пока нет | Нет встроенных адаптеров |

## Равенство

TypeScript-версия опирается на семантику Jotai / `Object.is`.
Rust runtime использует `PartialEq`. Две разные структуры с одинаковыми полями
обычно равны при `#[derive(PartialEq)]`, даже если выделены отдельно.
`Rc<T>` тоже обычно сравнивает содержимое; для pointer identity нужен собственный тип.

Это влияет на уведомления `prism` и стоимость сравнения крупных объектов.
`get_shared` уменьшает копирование, но не меняет выбранную семантику равенства.

## Типизированные props

Поля привязок назначаются Rust-замыканиями. Компилятор проверяет тип присваиваемого
значения, но Holo не знает, какое поле меняет замыкание. Поэтому он не обнаруживает
дублирование полей, не вычисляет оставшиеся обязательные public props и не переносит
аргументы компонента в атомы через committed `input`.

## События и async

Повторные одинаковые события доставляются отдельно. Вызов `Action` не порождает
событие автоматически. Bond не имеет output-атома, цепочек upstream-соединений,
readable/action triggers и обработчика `onError`.

`Action` может вернуть future как значение, но runtime его не запускает и не
отслеживает. Нет async atoms, Suspense, отмены, latest-wins или обработки async-ошибок.
Синхронная паника распространяется вызывающему коду; batch не откатывает записи.

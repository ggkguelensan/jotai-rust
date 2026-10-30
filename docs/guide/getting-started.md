# Быстрый старт

Создадим состояние счётчика и свяжем его с обычным Yew View.
Полный пример уже находится в `examples/yew-counter`.

## Запуск из репозитория

Нужны Git, stable Rust через rustup и Trunk. Установка rustup описана на
[rust-lang.org](https://www.rust-lang.org/tools/install).

```sh
git clone https://github.com/ggkguelensan/jotai-rust.git
cd jotai-rust
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd examples/yew-counter
trunk serve --address 127.0.0.1 --port 8080
```

Откройте <http://127.0.0.1:8080>. Два счётчика одной панели используют один Store.
Другая панель имеет собственные значения.

Crates в этом workspace подключаются через локальные path-зависимости.
Не используйте `cargo add jotai`: Rust-пакеты проекта пока не опубликованы.

## View, состояние и привязка

Ниже — самодостаточное приложение. Его можно использовать вместо
`examples/yew-counter/src/main.rs`.

```rust,no_run
use jotai::{Store, atom};
use jotai_holo::Action;
use jotai_holo_yew::{Holo, use_holo};
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
struct CounterProps {
    count: i32,
    on_increment: Callback<()>,
}

#[component]
fn CounterView(props: &CounterProps) -> Html {
    let increment = props.on_increment.clone();
    html! {
        <button onclick={move |_| increment.emit(())}>
            {format!("Счётчик: {}", props.count)}
        </button>
    }
}

fn counter_binding() -> Holo<CounterProps> {
    let count = atom(0);
    let increment = {
        let count = count.clone();
        Action::new(move |store, ()| store.update(&count, |n| n + 1))
    };
    Holo::new(|| CounterProps {
        count: 0,
        on_increment: Callback::noop(),
    })
    .read(&count, |props, value| props.count = value)
    .write(&increment, |props, callback| props.on_increment = callback)
}

#[component]
fn App() -> Html {
    let store = use_memo((), |_| Store::new());
    let binding = use_memo((), |_| counter_binding());
    let props = use_holo((*binding).clone(), (*store).clone());
    html! { <CounterView ..props /> }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
```

`read` подписывается на атом и присваивает его значение полю props.
`write` создаёт callback, вызывающий действие в выбранном Store.
`CounterView` получает только число и callback.

`use_memo` сохраняет идентичность Store и определения Holo между рендерами.
Создание новых атомов или привязок на каждом рендере меняет их идентичность.

## Следующие шаги

Прочитайте [о разделении модели и View](./mental-model.md), затем
[о владении подписками](./lifecycle.md). В [живом примере](../examples/counter.md)
действие проходит через событие и `Bond`, чтобы показать управление соединением.

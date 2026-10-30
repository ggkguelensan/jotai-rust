---
layout: home
hero:
  name: Jotai + Holo
  text: Атомы и обычные Views. На Rust.
  tagline: Храните логику в модели. Передавайте в Yew-компоненты значения и callbacks через декларативные привязки.
  actions:
    - theme: brand
      text: Начать
      link: /guide/getting-started
    - theme: alt
      text: Живой пример
      link: /examples/counter
features:
  - title: View получает обычные props
    details: Компонент не знает об атомах и Store. Его можно отрисовать напрямую с тестовыми значениями и callbacks.
  - title: Данные изолированы по Store
    details: Одно определение модели можно использовать в разных хранилищах. Значения, кеши и соединения остаются независимыми.
  - title: Владение явно
    details: Subscription и BondLease живут у владельца ресурса. Drop отключает подписку или освобождает соединение.
---

## Что попробовать

- [Открыть Excel-подобную таблицу](./examples/spreadsheet.md): формулы, несколько листов, клавиатурный редактор и локальное сохранение.
- [Связать модель с View](./guide/getting-started.md): первый счётчик без передачи атомов в компонент.
- [Посчитать корзину](./recipes/cart.md): несколько источников и независимые проекции.
- [Сохранить профиль](./recipes/profile.md): событие, guard, чтение состояния и действие через `Bond`.
- [Открыть WASM-демо](./examples/counter.md): два Store, повторные события и освобождение ресурсов.

::: info Статус
Это документация текущего синхронного MVP. Crates ещё не опубликованы.
Async, registry/lifecycle, формы и Query из TypeScript-версии пока не реализованы.
Точное соответствие описано в [таблице переноса](./guide/migration.md).
:::

## Как устроен проект

```text
jotai             определения атомов → Store → кеш и подписки
  jotai-holo      модели → Action / Event → Bond
    holo-yew      Holo.read / Holo.write → обычные props → View
```

Основа подхода — [Jotai](https://github.com/pmndrs/jotai) и
[jotai-holo](https://github.com/ggkguelensan/jotai-holo).
Синтаксис и контракты Rust-реализации местами отличаются; это самостоятельный runtime.

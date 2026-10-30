# Тестирование и сборка документации

Для тестирования модели браузер не нужен. Создайте Store, вызовите действие,
проверьте читаемые атомы. Короткие примеры в справочнике и рецептах выполняются
через rustdoc.

## Проверки Rust

Из корня репозитория:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --target wasm32-unknown-unknown
```

`cargo test --workspace` включает интеграционные тесты и doctests документации.
Отдельно выполнить только примеры из страниц:

```sh
npm run docs:check
```

Страницы подключаются к rustdoc при `cfg(doctest)` в соответствующем crate.
Руководство по Yew компилируется с `no_run`: Renderer запускается только в браузере.
Вставки исходников в живом примере проверяются сборкой самого WASM-приложения.

## Проверки в браузере

Node.js и npm нужны только для документации и Playwright, не для runtime.

```sh
npm ci
npx playwright install chromium
npm run test:browser
npm run test:docs
```

Playwright поднимает Trunk на `127.0.0.1:8080`, если сервер ещё не запущен.
Сценарий проверяет синхронизацию счётчиков, изоляцию Store, повторные события,
отключение соединения, remount и замену Store без JavaScript-ошибок.
`test:docs` отдельно собирает сайт и проверяет навигацию, поиск и встроенное WASM-демо.

## Сайт документации

```sh
npm ci
npm run docs:dev
```

Откройте адрес, напечатанный VitePress: при занятом порте сервер выберет следующий.
Команда сначала собирает Rust/WASM-демо,
затем запускает VitePress. Изменения Markdown видны сразу. После изменения
Rust-демо повторите `npm run docs:example`, чтобы обновить встроенную сборку.

Production-сборка и локальный просмотр:

```sh
npm run docs:check
npm run docs:build
npm run docs:preview
```

Просмотр доступен на <http://127.0.0.1:4173>.
`docs:build` собирает WASM и сайт; команда не публикует сайт на хостинг.

Для будущего размещения под префиксом, например GitHub Pages:

```sh
DOCS_BASE=/jotai-rust/ npm run docs:build
```

Готовые файлы находятся в `docs/.vitepress/dist`. Сгенерированные WASM/JS в
`docs/public/examples`, VitePress cache и dist исключены из Git. Их источник —
Rust-пример и Markdown, а не вручную редактируемые копии.

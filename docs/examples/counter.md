# Живой пример: два независимых Store

Это существующее Rust/Yew-приложение, собранное в WASM. Сайт загружает его в
iframe: действия и вычисления исполняются Rust runtime, без JavaScript-имитации.

<script setup>
import { withBase } from 'vitepress';
</script>

<iframe
  :src="withBase('/examples/counter/index.html')"
  title="Jotai + Holo: интерактивное Rust/WASM демо"
  style="width:100%;height:1120px;border:1px solid var(--vp-c-divider);border-radius:12px;background:#f4f4ef"
></iframe>

<a :href="withBase('/examples/counter/index.html')" target="_blank" rel="noopener">Открыть пример отдельно</a>

## Что проверить

| Действие | Результат |
| --- | --- |
| Increment в Store A | Оба счётчика A растут; Store B не меняется |
| Emit +1 twice | Два одинаковых события увеличивают число на 2 |
| Disconnect bond → Increment | Событие отправлено, но действие не выполняется |
| Connect bond → Increment | Соединение снова работает без сброса состояния |
| Hide derived → Increment → Show derived | Компонент восстанавливается с актуальным значением |
| Reset store | Панель получает новый Store, callbacks и подписки переподключаются |

Счётчик `Unrelated component renders` не растёт из-за изменения атома.
Локальные UI-действия в родительской панели могут инициировать другие рендеры
по обычным правилам Yew.

## Модель

<<< @/../examples/yew-counter/src/main.rs#counter-model

Модель содержит определения атомов и маршрут, но не значения конкретного Store.
`binding()` описывает, какие данные и команды получает View.

## Обычный View и соединяющий компонент

<<< @/../examples/yew-counter/src/main.rs#counter-view

<<< @/../examples/yew-counter/src/main.rs#counter-connected

## Кто владеет соединением

В полной реализации `StorePanel` хранит Store и подключает Bond в effect.
Переключение `connected` меняет владельца lease; очистка старого effect
освобождает прежний ресурс.

<<< @/../examples/yew-counter/src/main.rs#counter-ownership

Полный файл: [examples/yew-counter/src/main.rs](https://github.com/ggkguelensan/jotai-rust/blob/main/examples/yew-counter/src/main.rs).
Вставки исходников берутся из этого файла при сборке сайта.

import { defineConfig } from 'vitepress';

export default defineConfig({
  title: 'Jotai + Holo · Rust',
  description: 'Атомы, модели и обычные Yew Views. Руководство по Rust-адаптации Jotai и jotai-holo.',
  lang: 'ru-RU',
  base: process.env.DOCS_BASE || '/',
  lastUpdated: true,
  markdown: {
    config(md) {
      const fence = md.renderer.rules.fence;
      md.renderer.rules.fence = (tokens, index, ...args) => {
        // Keep rustdoc attributes in source while highlighting them as Rust on the site.
        if (tokens[index].info.startsWith('rust,')) tokens[index].info = 'rust';
        return fence(tokens, index, ...args);
      };
    },
  },
  themeConfig: {
    nav: [
      { text: 'Руководство', link: '/guide/getting-started' },
      { text: 'API', link: '/reference/atoms' },
      { text: 'Примеры', link: '/examples/counter' },
    ],
    sidebar: [
      { text: 'Начало', items: [
        { text: 'Быстрый старт', link: '/guide/getting-started' },
        { text: 'Модель и View', link: '/guide/mental-model' },
        { text: 'Владение и подписки', link: '/guide/lifecycle' },
        { text: 'Переход с TypeScript', link: '/guide/migration' },
      ] },
      { text: 'Справочник', items: [
        { text: 'atom, derived, ReadContext', link: '/reference/atoms' },
        { text: 'Store и Subscription', link: '/reference/store' },
        { text: 'Action и Event', link: '/reference/actions-events' },
        { text: 'ion, prism, Bond', link: '/reference/operators' },
        { text: 'Holo, Binding, use_holo', link: '/reference/holo' },
      ] },
      { text: 'Примеры и рецепты', items: [
        { text: 'Живой счётчик', link: '/examples/counter' },
        { text: 'Корзина', link: '/recipes/cart' },
        { text: 'Редактор профиля', link: '/recipes/profile' },
        { text: 'Тестирование', link: '/guide/testing' },
      ] },
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/ggkguelensan/jotai-rust' }],
    search: {
      provider: 'local',
      options: {
        translations: {
          button: { buttonText: 'Поиск', buttonAriaLabel: 'Поиск по документации' },
          modal: {
            displayDetails: 'Показать подробности',
            resetButtonTitle: 'Очистить поиск',
            backButtonTitle: 'Назад',
            noResultsText: 'Ничего не найдено',
            footer: { selectText: 'выбрать', navigateText: 'перейти', closeText: 'закрыть' },
          },
        },
      },
    },
    outline: { label: 'На этой странице', level: [2, 3] },
    docFooter: { prev: 'Назад', next: 'Далее' },
    sidebarMenuLabel: 'Разделы',
    returnToTopLabel: 'К началу',
    darkModeSwitchLabel: 'Тема',
    lastUpdated: { text: 'Обновлено' },
    editLink: {
      pattern: 'https://github.com/ggkguelensan/jotai-rust/edit/main/docs/:path',
      text: 'Редактировать на GitHub',
    },
    footer: { message: 'Экспериментальная Rust-адаптация. MIT. Не официальный релиз Jotai.' },
  },
});

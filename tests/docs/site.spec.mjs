import { test, expect } from '@playwright/test';

test('guide navigation, Rust snippets and local search', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Атомы и обычные Views. На Rust.' })).toBeVisible();
  await page.getByRole('link', { name: 'Начать', exact: true }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Быстрый старт');
  await expect(page.locator('.language-rust')).toHaveCount(1);
  await expect(page.locator('.language-rust')).toContainText('Renderer::<App>');

  await page.getByRole('button', { name: /Search|Поиск/ }).click();
  await page.locator('input[type="search"]').fill('Bond');
  await expect(page.locator('.VPLocalSearchBox .results')).toContainText('Bond');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('embedded WASM example works in the built documentation', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('examples/counter.html');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Живой пример: два независимых Store');
  await expect(page.locator('.vp-code').first()).toContainText('struct CounterModel');
  const demo = page.frameLocator('iframe');
  const a = demo.locator('section').nth(0);
  const b = demo.locator('section').nth(1);
  await expect(a.locator('.count')).toHaveText(['0', '0']);
  await a.locator('.increment').first().click();
  await expect(a.locator('.count')).toHaveText(['1', '1']);
  await expect(b.locator('.count')).toHaveText(['0', '0']);
  expect(errors).toEqual([]);
});

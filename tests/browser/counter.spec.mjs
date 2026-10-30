import { test, expect } from '@playwright/test';

test('shared counters, isolated stores, repeated events and owned connections', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  const a = page.locator('section').nth(0);
  const b = page.locator('section').nth(1);
  await expect(a.locator('.count')).toHaveText(['0', '0']);
  await expect(b.locator('.count')).toHaveText(['0', '0']);
  const unrelatedRenders = await a.locator('.independent').textContent();

  await a.locator('.increment').first().click();
  await expect(a.locator('.count')).toHaveText(['1', '1']);
  await expect(a.locator('.derived')).toHaveText('Double: 2 · odd');
  await expect(b.locator('.count')).toHaveText(['0', '0']);
  await expect(a.locator('.independent')).toHaveText(unrelatedRenders);

  await a.locator('.add-two').click();
  await expect(a.locator('.count')).toHaveText(['3', '3']);

  await a.locator('.toggle-connection').click();
  await a.locator('.increment').first().click();
  await expect(a.locator('.count')).toHaveText(['3', '3']);
  await b.locator('.increment').first().click();
  await expect(b.locator('.count')).toHaveText(['1', '1']);
  await a.locator('.toggle-connection').click();
  await a.locator('.increment').first().click();
  await expect(a.locator('.count')).toHaveText(['4', '4']);

  await a.locator('.toggle-derived').click();
  await expect(a.locator('.derived')).toHaveCount(0);
  await a.locator('.increment').first().click();
  await a.locator('.toggle-derived').click();
  await expect(a.locator('.derived')).toHaveText('Double: 10 · odd');

  await a.locator('.reset-store').click();
  await expect(a.locator('.count')).toHaveText(['0', '0']);
  await expect(a.locator('.derived')).toHaveText('Double: 0 · even');
  await a.locator('.increment').first().click();
  await expect(a.locator('.count')).toHaveText(['1', '1']);
  await expect(a.locator('.derived')).toHaveText('Double: 2 · odd');
  await expect(b.locator('.count')).toHaveText(['1', '1']);
  expect(errors).toEqual([]);
});

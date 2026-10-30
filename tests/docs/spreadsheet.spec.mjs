import { test, expect } from '@playwright/test';

async function open(page) {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('examples/spreadsheet.html');
  const app = page.frameLocator('iframe');
  await expect(app.locator('[data-address="D5"]')).toHaveValue('4136');
  return { app, errors };
}
const cell = (app, address) => app.locator(`[data-address="${address}"]`);
async function go(app, address) {
  await app.getByRole('textbox', { name: 'Перейти к ячейке', exact: true }).fill(address);
  await app.getByRole('button', { name: 'Перейти', exact: true }).click();
  await expect(cell(app, address)).toBeFocused();
}
async function edit(app, address, input, key = 'Enter') {
  await cell(app, address).dblclick();
  await cell(app, address).fill(input);
  await cell(app, address).press(key);
}

test('budget, formula bar, cancel, error values and cycle repair', async ({ page }) => {
  const { app, errors } = await open(page);
  await edit(app, 'B1', '20');
  await expect(cell(app, 'D1')).toHaveValue('1600');
  await expect(cell(app, 'D5')).toHaveValue('4840');
  await cell(app, 'D1').click();
  await expect(app.getByRole('textbox', { name: 'Формула', exact: true })).toHaveValue('=B1*C1');
  await edit(app, 'D1', '=1/0', 'Escape');
  await expect(cell(app, 'D1')).toHaveValue('1600');
  await edit(app, 'D1', '=1/0');
  await expect(cell(app, 'D1')).toHaveValue('#DIV/0!');
  await expect(cell(app, 'D5')).toHaveValue('#DIV/0!');
  await edit(app, 'D1', '=B1*C1');
  await expect(cell(app, 'D5')).toHaveValue('4840');
  await edit(app, 'A1', '=B1');
  await edit(app, 'B1', '=A1');
  await expect(cell(app, 'A1')).toHaveValue('#CYCLE!');
  await edit(app, 'B1', '20');
  await expect(cell(app, 'A1')).toHaveValue('20');

  // The formula bar is an editor, too; entering it must not move focus back into the grid.
  await cell(app, 'D1').click();
  const formula = app.getByRole('textbox', { name: 'Формула', exact: true });
  await formula.fill('=B1*2');
  await expect(formula).toBeFocused();
  await formula.press('Enter');
  await expect(cell(app, 'D1')).toHaveValue('40');
  expect(errors).toEqual([]);
});

test('fixed camera slots, offscreen formulas and dimension restoration', async ({ page }) => {
  const { app, errors } = await open(page);
  await cell(app, 'D5').click();
  const before = await cell(app, 'D5').boundingBox();
  await cell(app, 'D5').press('ArrowRight');
  await expect(cell(app, 'E5')).toBeFocused();
  const after = await cell(app, 'E5').boundingBox();
  expect(after.x).toBeCloseTo(before.x, 0);
  expect(after.y).toBeCloseTo(before.y, 0);
  await expect(app.locator('[data-address]')).toHaveCount(32);

  await go(app, 'AA90');
  await edit(app, 'AA90', '7');
  await go(app, 'A1');
  await edit(app, 'A1', '=AA90*3');
  await expect(cell(app, 'A1')).toHaveValue('21');
  const rows = app.getByRole('spinbutton', { name: 'Число строк', exact: true });
  await rows.fill('50'); await rows.press('Tab');
  await expect(cell(app, 'A1')).toHaveValue('#REF!');
  await rows.fill('100'); await rows.press('Tab');
  await expect(cell(app, 'A1')).toHaveValue('21');
  await go(app, 'AA90');
  await expect(cell(app, 'AA90')).toHaveValue('7');
  expect(errors).toEqual([]);
});

test('sheets commit drafts, preserve cameras, rename, add and restore locally', async ({ page }) => {
  const { app, errors } = await open(page);
  await cell(app, 'B1').dblclick(); await cell(app, 'B1').fill('20');
  await app.getByRole('button', { name: 'Черновик', exact: true }).click();
  await expect(cell(app, 'A1')).toHaveValue('');
  await edit(app, 'A1', '=2+3');
  await go(app, 'AA90'); await edit(app, 'AA90', '42');
  await app.getByRole('button', { name: 'Черновик', exact: true }).dblclick();
  await app.getByRole('textbox', { name: 'Название листа' }).fill('Заметки');
  await app.getByRole('textbox', { name: 'Название листа' }).press('Enter');
  await expect(app.getByRole('button', { name: 'Заметки', exact: true })).toBeVisible();
  await app.getByRole('button', { name: 'Бюджет подарков', exact: true }).click();
  await expect(cell(app, 'B1')).toHaveValue('20');
  await expect(cell(app, 'D5')).toHaveValue('4840');
  await app.getByRole('button', { name: 'Заметки', exact: true }).click();
  await expect(cell(app, 'AA91')).toBeFocused();
  await expect(cell(app, 'AA90')).toHaveValue('42');
  await expect(app.getByRole('status')).toHaveText('✓ Сохранено локально');
  await page.reload();
  await expect(app.getByRole('button', { name: 'Заметки', exact: true })).toHaveAttribute('aria-current', 'page');
  await expect(cell(app, 'AA90')).toHaveValue('42');
  await go(app, 'A1'); await expect(cell(app, 'A1')).toHaveValue('5');
  await app.getByRole('button', { name: 'Добавить лист' }).click();
  await expect(app.getByRole('button', { name: 'Лист 3', exact: true })).toHaveAttribute('aria-current', 'page');
  await expect(cell(app, 'A1')).toHaveValue('');
  await edit(app, 'A1', '9');
  await app.getByRole('button', { name: 'Сбросить лист' }).click();
  await expect(cell(app, 'A1')).toHaveValue('');
  expect(errors).toEqual([]);
});

test('narrow viewport mounts two columns and stays within screen bounds', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 900 });
  const { app, errors } = await open(page);
  await expect(app.locator('[data-address]')).toHaveCount(16);
  const overflow = await app.locator('body').evaluate(body => body.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await go(app, 'AA90'); await edit(app, 'AA90', '6');
  await expect(cell(app, 'AA90')).toHaveValue('6');
  await app.getByRole('button', { name: 'Сбросить лист' }).click();
  await expect(app.locator('[data-address]')).toHaveCount(16);
  await expect(cell(app, 'D5')).toHaveValue('4136');
  expect(errors).toEqual([]);
});

test('storage failure preserves edits and retry writes the current workbook', async ({ page }) => {
  await page.addInitScript(() => {
    const original = Storage.prototype.setItem;
    window.restoreStorageWrites = () => { Storage.prototype.setItem = original; };
    Storage.prototype.setItem = () => { throw new DOMException('Full', 'QuotaExceededError'); };
  });
  const { app, errors } = await open(page);
  await edit(app, 'B1', '20');
  await expect(app.getByRole('alert')).toContainText('Не удалось сохранить');
  await expect(cell(app, 'D5')).toHaveValue('4840');
  await app.locator('body').evaluate(() => window.restoreStorageWrites());
  await app.getByRole('button', { name: 'Повторить', exact: true }).click();
  await expect(app.getByRole('status')).toHaveText('✓ Сохранено локально');
  await page.reload();
  await expect(cell(app, 'B1')).toHaveValue('20');
  await expect(cell(app, 'D5')).toHaveValue('4840');
  expect(errors).toEqual([]);
});

import { waitForAppReady } from '../../helpers/app.js';
import composePage from '../../pageobjects/compose.page.js';

describe('Compose Email', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  afterEach(async () => {
    await composePage.closeIfOpen();
  });

  it('打开写邮件弹窗并填写字段', async () => {
    await composePage.openCompose();
    await composePage.fillEmail('recipient.e2e@postium.test', 'E2E Compose Subject');

    await expect(await composePage.recipientText()).toContain('recipient.e2e@postium.test');
    await expect(composePage.subjectInput).toHaveValue('E2E Compose Subject');

    await composePage.closeCompose();
  });

  it('重新打开写邮件弹窗时字段为空', async () => {
    await composePage.openCompose();

    await expect(composePage.toInput).toHaveValue('');
    await expect(composePage.subjectInput).toHaveValue('');

    await composePage.closeCompose();
  });
});

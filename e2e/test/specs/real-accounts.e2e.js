import path from 'path';
import { fileURLToPath } from 'url';
import { loadTruthAccounts } from '../../helpers/truth-config.js';
import { waitForTruthAppShell } from '../../helpers/app.js';
import settingsPage from '../../pageobjects/settings.page.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';
import emailPage from '../../pageobjects/email.page.js';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const rootDir = path.resolve(__dirname, '../../..');
const accounts = loadTruthAccounts(rootDir);

describe('Truth Real Mail Accounts', () => {
  before(async () => {
    await waitForTruthAppShell();
  });

  for (const account of accounts) {
    it(`添加真实账号并验证基础交互: ${account.key}`, async () => {
      await settingsPage.addManualAccount(account);

      const exists = await settingsPage.hasAccount(account.email);
      await expect(exists).toBe(true);

      await sidebarPage.switchToAccountByEmail(account.email);
      await sidebarPage.clickInbox();
      await emailPage.waitForReady();

      if (await emailPage.hasEmails()) {
        await emailPage.clickEmail(0);
        await emailPage.detailPanel.waitForDisplayed({ timeout: 10000 });
      } else {
        await emailPage.detailEmpty.waitForDisplayed({ timeout: 10000 });
      }
    });
  }
});

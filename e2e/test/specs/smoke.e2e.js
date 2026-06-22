import { waitForAppReady } from '../../helpers/app.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';
import emailPage from '../../pageobjects/email.page.js';

describe('Smoke', () => {
  it('启动应用并显示主账号 inbox seed 数据', async () => {
    await waitForAppReady();
    expect(await sidebarPage.activeAccountText()).toContain('primary.e2e@postium.test');
    await expect(emailPage.list).toBeDisplayed();
  });
});

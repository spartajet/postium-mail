import { waitForText } from '../../helpers/selectors.js';
import { waitForAppReady } from '../../helpers/app.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Navigation', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  it('切换到 Sent 并显示已发送邮件', async () => {
    await sidebarPage.clickSent();
    await waitForText('Sent Confirmation Message');
  });

  it('切换到 Starred 并显示星标邮件', async () => {
    await sidebarPage.clickStarred();
    await waitForText('Starred Reference Message');
  });

  it('切回 Inbox 并显示主账号 inbox 邮件', async () => {
    await sidebarPage.clickInbox();
    await waitForText('Primary Inbox Message 01');
  });
});

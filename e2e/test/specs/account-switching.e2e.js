import { waitForAppReady } from '../../helpers/app.js';
import { waitForText, waitForTextGone } from '../../helpers/selectors.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Account switching', () => {
  it('在主账号和次账号之间切换并刷新邮件列表', async () => {
    await waitForAppReady();
    await sidebarPage.switchToSecondary();
    await waitForText('Secondary Inbox Message 01');
    await waitForTextGone('Primary Inbox Message 01');

    await sidebarPage.switchToPrimary();
    await waitForText('Primary Inbox Message 01');
  });
});

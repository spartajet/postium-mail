import sidebarPage from '../pageobjects/sidebar.page.js';
import emailPage from '../pageobjects/email.page.js';
import { waitForText } from './selectors.js';

export async function waitForAppReady() {
  await sidebarPage.waitForReady();
  if (!(await sidebarPage.activeAccountText()).includes('primary.e2e@postium.test')) {
    await sidebarPage.switchToPrimary();
  }
  await sidebarPage.clickInbox();
  await emailPage.waitForReady();
  await emailPage.clearSearch();
  await waitForText('Primary Inbox Message 01');
}

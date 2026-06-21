import sidebarPage from '../pageobjects/sidebar.page.js';
import emailPage from '../pageobjects/email.page.js';
import { waitForText } from './selectors.js';

export async function waitForAppReady() {
  await sidebarPage.waitForReady();
  await emailPage.waitForReady();
  await waitForText('Primary Inbox Message 01');
}

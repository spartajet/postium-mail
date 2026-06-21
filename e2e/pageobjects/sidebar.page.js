import { byTestId } from '../helpers/selectors.js';

class SidebarPage {
  get root() { return byTestId('sidebar'); }
  get composeButton() { return byTestId('compose-button'); }
  get inboxFolder() { return byTestId('folder-inbox'); }
  get sentFolder() { return byTestId('folder-sent'); }
  get starredFolder() { return byTestId('folder-starred'); }
  get settingsButton() { return byTestId('settings-nav'); }
  get accountSwitcher() { return byTestId('account-switcher'); }
  get primaryAccountOption() { return byTestId('account-option-primary'); }
  get secondaryAccountOption() { return byTestId('account-option-secondary'); }
  get activeAccountLabel() { return byTestId('active-account-label'); }
  get folderItems() { return $$('[data-testid^="folder-"]'); }

  async waitForReady() {
    await this.root.waitForDisplayed({ timeout: 10000 });
    await this.accountSwitcher.waitForDisplayed({ timeout: 10000 });
    await this.inboxFolder.waitForDisplayed({ timeout: 10000 });
  }

  async openAccountSwitcher() {
    await this.accountSwitcher.waitForDisplayed({ timeout: 10000 });
    await this.accountSwitcher.click();
  }

  async switchToPrimary() {
    await this.openAccountSwitcher();
    await this.primaryAccountOption.waitForDisplayed({ timeout: 10000 });
    await this.primaryAccountOption.click();
  }

  async switchToSecondary() {
    await this.openAccountSwitcher();
    await this.secondaryAccountOption.waitForDisplayed({ timeout: 10000 });
    await this.secondaryAccountOption.click();
  }

  async clickInbox() {
    await this.inboxFolder.waitForDisplayed({ timeout: 10000 });
    await this.inboxFolder.click();
  }

  async clickSent() {
    await this.sentFolder.waitForDisplayed({ timeout: 10000 });
    await this.sentFolder.click();
  }

  async clickStarred() {
    await this.starredFolder.waitForDisplayed({ timeout: 10000 });
    await this.starredFolder.click();
  }

  async clickSettings() {
    await this.settingsButton.waitForDisplayed({ timeout: 10000 });
    await this.settingsButton.click();
  }

  async clickFolder(folderName) {
    const folder = byTestId(`folder-${folderName.toLowerCase()}`);
    await folder.waitForDisplayed({ timeout: 10000 });
    await folder.click();
  }
}

export default new SidebarPage();

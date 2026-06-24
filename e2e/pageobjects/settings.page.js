import sidebarPage from './sidebar.page.js';
import { byTestId } from '../helpers/selectors.js';

class SettingsPage {
  get page() { return byTestId('settings-page'); }
  get settingsNavButton() { return byTestId('settings-nav'); }
  get lightThemeButton() { return byTestId('theme-light'); }
  get darkThemeButton() { return byTestId('theme-dark'); }
  get systemThemeButton() { return byTestId('theme-system'); }
  get themeButtons() { return $$('[data-testid^="theme-"]'); }
  get addAccountModal() { return byTestId('add-account-modal'); }
  get addAccountCloseButton() { return byTestId('add-account-close-button'); }
  get providerOtherButton() { return byTestId('add-account-provider-other-button'); }
  get emailInput() { return byTestId('add-account-email-input'); }
  get displayNameInput() { return byTestId('add-account-display-name-input'); }
  get passwordInput() { return byTestId('add-account-password-input'); }
  get imapHostInput() { return byTestId('add-account-imap-host-input'); }
  get imapPortInput() { return byTestId('add-account-imap-port-input'); }
  get imapSslSelect() { return byTestId('add-account-imap-ssl-select'); }
  get smtpHostInput() { return byTestId('add-account-smtp-host-input'); }
  get smtpPortInput() { return byTestId('add-account-smtp-port-input'); }
  get smtpSslSelect() { return byTestId('add-account-smtp-ssl-select'); }
  get submitButton() { return byTestId('add-account-submit-button'); }
  get errorMessage() { return byTestId('add-account-error'); }
  get donePanel() { return byTestId('add-account-done'); }
  get doneCloseButton() { return byTestId('add-account-done-close-button'); }
  get accountCards() { return $$('[data-testid="settings-account-card"]'); }

  async waitForReady() {
    await this.page.waitForDisplayed({ timeout: 10000 });
  }

  async navigateToSettings() {
    await sidebarPage.clickSettings();
    await this.waitForReady();
  }

  async isDarkMode() {
    const html = await $('html');
    const classes = await html.getAttribute('class');
    return classes ? classes.includes('dark') : false;
  }

  async addManualAccount(account) {
    await sidebarPage.openAddAccountModal();
    await this.addAccountModal.waitForDisplayed({ timeout: 10000 });
    await this.providerOtherButton.waitForDisplayed({ timeout: 10000 });
    await this.providerOtherButton.click();
    await this.emailInput.waitForDisplayed({ timeout: 10000 });
    await this.emailInput.setValue(account.email);
    await this.displayNameInput.setValue(`Truth ${account.key}`);
    await this.passwordInput.setValue(account.password);
    await this.imapHostInput.setValue(account.imap.host);
    await this.imapPortInput.setValue(String(account.imap.port));
    await this.imapSslSelect.selectByAttribute('value', account.imap.ssl ? 'Tls' : 'None');
    await this.smtpHostInput.setValue(account.smtp.host);
    await this.smtpPortInput.setValue(String(account.smtp.port));
    await this.smtpSslSelect.selectByAttribute('value', account.smtp.ssl ? 'Tls' : 'None');
    await this.submitButton.click();

    const completed = await browser.waitUntil(
      async () => (
        await this.donePanel.isExisting()
        || await this.errorMessage.isExisting()
      ),
      {
        timeout: 120000,
        timeoutMsg: `添加真实账号 ${account.key} 超时`,
      },
    );

    if (!completed || await this.errorMessage.isExisting()) {
      const message = await this.errorMessage.getText();
      await this.addAccountCloseButton.click();
      await this.addAccountModal.waitForExist({ reverse: true, timeout: 10000 });
      throw new Error(`添加真实账号 ${account.key} 失败: ${message}`);
    }

    await this.donePanel.waitForDisplayed({ timeout: 10000 });
    await this.doneCloseButton.click();
    await this.donePanel.waitForExist({ reverse: true, timeout: 10000 });
  }

  async hasAccount(email) {
    await this.navigateToSettings();
    const escapedEmail = email.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
    const selector = `[data-testid="settings-account-card"][data-email="${escapedEmail}"]`;
    const card = await $(selector);
    return card.isExisting();
  }
}

export default new SettingsPage();

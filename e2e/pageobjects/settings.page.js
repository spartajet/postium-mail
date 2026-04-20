/**
 * Settings page object for Postium Mail
 * Handles theme switching and account management
 */
class SettingsPage {
  get settingsNavButton() {
    return $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('设置') || text.includes('Settings');
    });
  }

  // Theme buttons
  get themeButtons() {
    return $$('button').filter(async (btn) => {
      const text = await btn.getText();
      return (
        text.includes('浅色') || text.includes('Light') ||
        text.includes('深色') || text.includes('Dark') ||
        text.includes('系统') || text.includes('System')
      );
    });
  }

  get lightThemeButton() {
    return $$('button').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('浅色') || text.includes('Light');
    });
  }

  get darkThemeButton() {
    return $$('button').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('深色') || text.includes('Dark');
    });
  }

  get systemThemeButton() {
    return $$('button').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('系统') || text.includes('System');
    });
  }

  // Account management
  get addAccountButton() {
    return $$('button').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('添加账号') || text.includes('Add Account');
    });
  }

  get accountCards() {
    return $$('div.flex.items-center.gap-3.rounded-lg.border');
  }

  async navigateToSettings() {
    const btn = await this.settingsNavButton;
    if (btn) {
      await btn.click();
      await browser.pause(1000);
    }
  }

  async isDarkMode() {
    const html = await $('html');
    const classes = await html.getAttribute('class');
    return classes ? classes.includes('dark') : false;
  }
}

export default new SettingsPage();

import sidebarPage from './sidebar.page.js';
import { byTestId } from '../helpers/selectors.js';

class SettingsPage {
  get page() { return byTestId('settings-page'); }
  get settingsNavButton() { return byTestId('settings-nav'); }
  get lightThemeButton() { return byTestId('theme-light'); }
  get darkThemeButton() { return byTestId('theme-dark'); }
  get systemThemeButton() { return byTestId('theme-system'); }
  get themeButtons() { return $$('[data-testid^="theme-"]'); }

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
}

export default new SettingsPage();

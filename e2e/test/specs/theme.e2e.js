import settingsPage from '../../pageobjects/settings.page.js';

describe('Theme Switching', () => {
  beforeEach(async () => {
    await settingsPage.openInCurrentWindow();
  });

  it('切换到深色主题', async () => {
    await settingsPage.darkThemeButton.click();
    await browser.waitUntil(() => settingsPage.isDarkMode(), {
      timeout: 5000,
      timeoutMsg: '深色主题未生效',
    });
  });

  it('切换到浅色主题', async () => {
    await settingsPage.lightThemeButton.click();
    await browser.waitUntil(async () => !(await settingsPage.isDarkMode()), {
      timeout: 5000,
      timeoutMsg: '浅色主题未生效',
    });
  });

  it('系统主题按钮可点击', async () => {
    await settingsPage.systemThemeButton.click();
    await expect(settingsPage.systemThemeButton).toBeDisplayed();
  });
});

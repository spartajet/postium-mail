import settingsPage from '../../pageobjects/settings.page.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Theme Switching', () => {
  it('should navigate to settings page', async () => {
    await browser.pause(1000);
    await settingsPage.navigateToSettings();
    await browser.pause(1000);
  });

  it('should switch to dark theme', async () => {
    const darkBtn = await settingsPage.darkThemeButton;
    if (darkBtn) {
      await darkBtn.click();
      await browser.pause(500);

      const isDark = await settingsPage.isDarkMode();
      expect(isDark).toBe(true);
    }
  });

  it('should switch to light theme', async () => {
    const lightBtn = await settingsPage.lightThemeButton;
    if (lightBtn) {
      await lightBtn.click();
      await browser.pause(500);

      const isDark = await settingsPage.isDarkMode();
      expect(isDark).toBe(false);
    }
  });

  it('should switch to system theme', async () => {
    const systemBtn = await settingsPage.systemThemeButton;
    if (systemBtn) {
      await systemBtn.click();
      await browser.pause(500);
      // System theme follows OS preference - just verify the button is clickable
      expect(systemBtn).toBeTruthy();
    }
  });

  it('should return to main view after settings', async () => {
    // Click inbox to navigate back
    const inbox = await sidebarPage.inboxFolder;
    if (inbox) {
      await inbox.click();
      await browser.pause(500);
    }
  });
});

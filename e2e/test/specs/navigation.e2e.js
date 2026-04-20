import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Navigation', () => {
  it('should launch the app and show the main window', async () => {
    // The app should be running after wdio launches it
    const title = await browser.getTitle();
    expect(title).toBeTruthy();
  });

  it('should display sidebar navigation items', async () => {
    const items = await sidebarPage.folderItems;
    expect(items.length).toBeGreaterThan(0);
  });

  it('should show folder navigation with at least inbox', async () => {
    // Wait for the sidebar to render
    await browser.pause(1000);
    const inbox = await sidebarPage.inboxFolder;
    expect(inbox).toBeTruthy();
  });
});

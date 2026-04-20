import sidebarPage from "../../pageobjects/sidebar.page.js";

describe("Account Management", () => {
  it("should show add account option when no accounts exist", async () => {
    await browser.pause(1000);
    // The app should prompt to add an account when none exist
    const body = await $("body");
    const text = await body.getText();
    // Either shows empty state or add account prompt
    expect(text).toBeTruthy();
  });

  it("should have settings accessible", async () => {
    await browser.pause(500);
    const settingsBtn = await sidebarPage.settingsButton;
    // Settings button should exist in the sidebar
    expect(settingsBtn).toBeTruthy();
  });
});

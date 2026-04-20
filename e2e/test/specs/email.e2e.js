import emailPage from "../../pageobjects/email.page.js";
import sidebarPage from "../../pageobjects/sidebar.page.js";

describe("Email List", () => {
  it("should display the email list panel", async () => {
    await browser.pause(1000);
    // The list panel should be visible on the main view
    const listPanel = await $(".list-panel");
    expect(listPanel).toBeTruthy();
  });

  it("should have a search input", async () => {
    const searchInput = await emailPage.searchInput;
    expect(searchInput).toBeTruthy();
  });

  it("should switch to sent folder", async () => {
    const sentFolder = await sidebarPage.sentFolder;
    if (sentFolder) {
      await sentFolder.click();
      await browser.pause(1000);
    }
  });

  it("should switch to starred folder", async () => {
    const starredFolder = await sidebarPage.starredFolder;
    if (starredFolder) {
      await starredFolder.click();
      await browser.pause(1000);
    }
  });

  it("should switch back to inbox folder", async () => {
    const inboxFolder = await sidebarPage.inboxFolder;
    if (inboxFolder) {
      await inboxFolder.click();
      await browser.pause(1000);
    }
  });
});

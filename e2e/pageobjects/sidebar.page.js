/**
 * Sidebar page object for Postium Mail
 * Handles folder navigation and sidebar interactions
 */
class SidebarPage {
  get settingsButton() {
    return $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('设置') || text.includes('Settings');
    });
  }

  get folderItems() {
    return $$('.nav-item');
  }

  get inboxFolder() {
    return $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('收件箱') || text.includes('Inbox');
    });
  }

  get starredFolder() {
    return $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('星标') || text.includes('Starred');
    });
  }

  get sentFolder() {
    return $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('已发送') || text.includes('Sent');
    });
  }

  async clickFolder(folderName) {
    const folder = await $$('button.nav-item').find(async (btn) => {
      const text = await btn.getText();
      return text.includes(folderName);
    });
    if (folder) {
      await folder.click();
    }
  }

  async clickSettings() {
    const btn = await this.settingsButton;
    if (btn) {
      await btn.click();
    }
  }
}

export default new SidebarPage();

/**
 * Email page object for Postium Mail
 * Handles email list, detail view, and search interactions
 */
class EmailPage {
  get emailItems() {
    return $$('.email-item');
  }

  get activeEmailItem() {
    return $('.email-item.active');
  }

  get searchInput() {
    return $('input[placeholder*="搜索"], input[placeholder*="Search"]').catch(() => {
      return $('input').find(async (el) => {
        const placeholder = await el.getAttribute('placeholder');
        return placeholder && (placeholder.includes('搜索') || placeholder.includes('Search'));
      });
    });
  }

  get searchContainer() {
    return $('.search-container');
  }

  // Email detail view
  get detailPanel() {
    return $('.detail-panel');
  }

  get detailSubject() {
    return $('.detail-panel .text-xl');
  }

  get detailSender() {
    return $('.detail-panel .text-sm.font-medium');
  }

  // Action buttons in detail view
  get replyButton() {
    return $$('button').find(async (btn) => {
      const label = await btn.getAttribute('aria-label');
      return label && (label.includes('回复') || label.includes('Reply'));
    });
  }

  get forwardButton() {
    return $$('button').find(async (btn) => {
      const label = await btn.getAttribute('aria-label');
      return label && (label.includes('转发') || label.includes('Forward'));
    });
  }

  get starButton() {
    return $$('button').find(async (btn) => {
      const label = await btn.getAttribute('aria-label');
      return label && (label.includes('星标') || label.includes('Star'));
    });
  }

  get deleteButton() {
    return $$('button').find(async (btn) => {
      const label = await btn.getAttribute('aria-label');
      return label && (label.includes('删除') || label.includes('Delete'));
    });
  }

  async clickEmail(index = 0) {
    const items = await this.emailItems;
    if (items.length > index) {
      await items[index].click();
      await browser.pause(500);
    }
  }

  async search(query) {
    const input = await this.searchInput;
    if (input) {
      await input.setValue(query);
      await browser.pause(500);
    }
  }

  async hasEmails() {
    const items = await this.emailItems;
    return items.length > 0;
  }
}

export default new EmailPage();

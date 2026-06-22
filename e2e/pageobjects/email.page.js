import {
  allByTestId,
  byTestId,
  elementText,
  pageText,
  waitForText,
} from '../helpers/selectors.js';

class EmailPage {
  get list() { return byTestId('email-list'); }
  get searchInput() { return byTestId('email-search-input'); }
  get resultCount() { return byTestId('email-result-count'); }
  get emailItems() { return allByTestId('email-item'); }
  get activeEmailItem() { return $('[data-testid="email-item"].active'); }
  get detail() { return byTestId('email-detail'); }
  get detailEmpty() { return byTestId('email-detail-empty'); }
  get detailPanel() { return this.detail; }
  get detailSubject() { return byTestId('email-subject'); }
  get detailSender() { return byTestId('email-sender'); }
  get detailBody() { return byTestId('email-body'); }
  get starButton() { return byTestId('email-star-button'); }
  get deleteButton() { return byTestId('email-delete-button'); }
  get searchContainer() { return $('.search-container'); }

  async waitForReady() {
    await this.list.waitForDisplayed({ timeout: 10000 });
  }

  async search(query) {
    await this.searchInput.waitForDisplayed({ timeout: 10000 });
    await this.searchInput.setValue(query);
    await this.waitForSearchMode();
    await waitForText(query.split(' ')[0]);
  }

  async waitForSearchMode() {
    await browser.waitUntil(
      async () => (await this.list.getAttribute('data-search-mode')) === 'true',
      {
        timeout: 10000,
        timeoutMsg: '邮件列表未进入搜索模式',
      }
    );
  }

  async resultCountText() {
    await this.resultCount.waitForDisplayed({ timeout: 10000 });
    return elementText(await this.resultCount);
  }

  async clearSearch() {
    await this.searchInput.waitForDisplayed({ timeout: 10000 });
    await browser.execute(() => {
      // eslint-disable-next-line no-undef
      const input = document.querySelector('[data-testid="email-search-input"]');
      if (!input) return;
      input.value = '';
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
  }

  async clickEmail(index = 0) {
    const items = await this.emailItems;
    if (items.length <= index) {
      throw new Error(`邮件列表数量不足，无法点击索引 ${index}`);
    }
    await items[index].click();
    await this.detail.waitForDisplayed({ timeout: 10000 });
  }

  async clickEmailBySubject(subject) {
    await waitForText(subject);
    const item = await this.findEmailBySubject(subject);
    await item.click();
    await this.detail.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => (await this.detailSubject.getText()).includes(subject),
      {
        timeout: 10000,
        timeoutMsg: `邮件详情未显示主题: ${subject}`,
      }
    );
  }

  async isStarredInDetail() {
    await this.starButton.waitForDisplayed({ timeout: 10000 });
    return (await this.starButton.getAttribute('data-starred')) === 'true';
  }

  async toggleStarInDetail() {
    await this.starButton.waitForDisplayed({ timeout: 10000 });
    await this.starButton.click();
    await browser.waitUntil(
      async () => (await this.isStarredInDetail()) === true,
      {
        timeout: 10000,
        timeoutMsg: '邮件详情星标状态未更新为已星标',
      }
    );
  }

  async deleteOpenEmail() {
    await this.deleteButton.waitForDisplayed({ timeout: 10000 });
    await this.deleteButton.click();
    await this.detailEmpty.waitForDisplayed({ timeout: 10000 });
  }

  async findEmailBySubject(subject) {
    const itemBySubject = await $(`[data-testid="email-item"][data-subject="${subject}"]`);
    if (await itemBySubject.isExisting()) return itemBySubject;

    const items = await this.emailItems;
    for (const item of items) {
      if (!(await item.isExisting())) continue;

      const dataSubject = await item.getAttribute('data-subject').catch(() => '');
      if (dataSubject === subject) return item;

      const text = await item.getText().catch(() => '');
      if (text.includes(subject)) return item;
    }
    throw new Error(`未找到邮件: ${subject}`);
  }

  async scrollToSubject(subject) {
    await this.list.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => {
        const bodyText = await pageText();
        if (bodyText.includes(subject)) return true;
        await browser.execute(() => {
          // eslint-disable-next-line no-undef
          const list = document.querySelector('[data-testid="email-list"] .overflow-y-auto');
          list?.scrollBy(0, 500);
        });
        return false;
      },
      {
        timeout: 10000,
        timeoutMsg: `滚动列表后仍未找到邮件: ${subject}`,
      }
    );
  }

  async hasEmails() {
    const items = await this.emailItems;
    return items.length > 0;
  }

  async detailSubjectText() {
    await this.detailSubject.waitForDisplayed({ timeout: 10000 });
    return elementText(await this.detailSubject);
  }
}

export default new EmailPage();

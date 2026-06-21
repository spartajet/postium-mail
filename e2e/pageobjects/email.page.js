import { allByTestId, byTestId, waitForText } from '../helpers/selectors.js';

class EmailPage {
  get list() { return byTestId('email-list'); }
  get searchInput() { return byTestId('email-search-input'); }
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
    await waitForText(query.split(' ')[0]);
  }

  async clearSearch() {
    await this.searchInput.waitForDisplayed({ timeout: 10000 });
    await this.searchInput.setValue('');
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

  async findEmailBySubject(subject) {
    const items = await this.emailItems;
    for (const item of items) {
      const text = await item.getText();
      if (text.includes(subject)) return item;
    }
    throw new Error(`未找到邮件: ${subject}`);
  }

  async scrollToSubject(subject) {
    await this.list.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => {
        const bodyText = await $('body').getText();
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
}

export default new EmailPage();

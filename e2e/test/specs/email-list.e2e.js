import { waitForAppReady } from '../../helpers/app.js';
import { waitForText, waitForTextGone } from '../../helpers/selectors.js';
import emailPage from '../../pageobjects/email.page.js';

describe('Email list', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  it('搜索 Quarterly Planning 并显示固定结果', async () => {
    await emailPage.search('Quarterly Planning');
    expect(await emailPage.resultCountText()).toContain('3');
    await waitForText('Quarterly Planning Alpha');
    await waitForText('Quarterly Planning Beta');
    await waitForText('Quarterly Planning Archive');
  });

  it('滚动到 inbox 底部邮件并打开详情', async () => {
    await emailPage.scrollToSubject('Primary Inbox Message 24');
    await emailPage.clickEmailBySubject('Primary Inbox Message 24');
    expect(await emailPage.detailSubjectText()).toContain('Primary Inbox Message 24');
  });

  it('在详情页切换星标并删除当前邮件', async () => {
    const subject = 'Primary Inbox Message 23';

    await emailPage.scrollToSubject(subject);
    await emailPage.clickEmailBySubject(subject);
    expect(await emailPage.isStarredInDetail()).toBe(false);

    await emailPage.toggleStarInDetail();
    expect(await emailPage.isStarredInDetail()).toBe(true);

    await emailPage.deleteOpenEmail();
    await waitForTextGone(subject);
  });
});

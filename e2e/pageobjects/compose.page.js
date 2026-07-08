import { byTestId, elementText } from '../helpers/selectors.js';

class ComposePage {
  get composeButton() { return byTestId('compose-button'); }
  get modal() { return byTestId('compose-modal'); }
  get closeButton() { return byTestId('compose-close-button'); }
  get toRecipientField() { return byTestId('to-recipient-field'); }
  get toInput() { return byTestId('to-recipient-input'); }
  get ccInput() { return byTestId('compose-cc-input'); }
  get subjectInput() { return byTestId('compose-subject-input'); }
  get bodyEditorWrap() { return byTestId('compose-body-editor'); }
  get sendButton() { return byTestId('compose-send-button'); }
  get cancelButton() { return this.modal.$$('button').find(async (btn) => {
    const text = await btn.getText();
    return text.includes('取消') || text.includes('Cancel');
  }); }

  async openCompose() {
    await this.composeButton.waitForDisplayed({ timeout: 10000 });
    await this.composeButton.click();
    await this.modal.waitForDisplayed({ timeout: 10000 });
  }

  async closeCompose() {
    await this.closeButton.waitForDisplayed({ timeout: 10000 });
    await this.closeButton.click();
    await this.modal.waitForExist({ reverse: true, timeout: 10000 });
  }

  async closeIfOpen() {
    if (await this.modal.isExisting()) {
      await this.closeCompose();
    }
  }

  async fillEmail(to, subject) {
    await this.toInput.waitForDisplayed({ timeout: 10000 });
    await this.toInput.setValue(to);
    await browser.keys('Enter');
    await browser.waitUntil(
      async () => (await elementText(await this.toRecipientField)).includes(to),
      {
        timeout: 10000,
        timeoutMsg: `收件人 chip 未显示: ${to}`,
      }
    );
    await this.subjectInput.setValue(subject);
  }

  async recipientText() {
    await this.toRecipientField.waitForDisplayed({ timeout: 10000 });
    return elementText(await this.toRecipientField);
  }

  async bodyEditor() {
    await this.bodyEditorWrap.waitForDisplayed({ timeout: 10000 });
    return this.bodyEditorWrap.$('.ProseMirror');
  }

  async send() {
    await this.sendButton.waitForDisplayed({ timeout: 10000 });
    await this.sendButton.click();
  }
}

export default new ComposePage();

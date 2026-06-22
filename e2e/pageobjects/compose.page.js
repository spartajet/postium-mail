import { byTestId } from '../helpers/selectors.js';

class ComposePage {
  get composeButton() { return byTestId('compose-button'); }
  get modal() { return byTestId('compose-modal'); }
  get closeButton() { return byTestId('compose-close-button'); }
  get toInput() { return byTestId('compose-to-input'); }
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

  async fillEmail(to, subject) {
    await this.toInput.waitForDisplayed({ timeout: 10000 });
    await this.toInput.setValue(to);
    await this.subjectInput.setValue(subject);
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

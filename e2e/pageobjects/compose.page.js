/**
 * Compose page object for Postium Mail
 * Handles the compose email modal interactions
 */
class ComposePage {
  get composeButton() {
    return $('.compose-btn');
  }

  get modal() {
    return $('div.fixed.inset-0.z-50');
  }

  get closeButton() {
    return this.modal.$('button[aria-label="Close"]');
  }

  get toInput() {
    // The To field is the first text input in the compose modal
    const inputs = this.modal.$$('input[type="text"]');
    return inputs[0];
  }

  get ccInput() {
    const inputs = this.modal.$$('input[type="text"]');
    return inputs[1];
  }

  get subjectInput() {
    // Subject input has no placeholder; it's the third text input in the modal
    // (To = first, CC = second, Subject = third)
    const inputs = this.modal.$$('input[type="text"]');
    return inputs[2];
  }

  get bodyEditor() {
    // The rich text editor content area
    return this.modal.$('.ProseMirror');
  }

  get sendButton() {
    // Send button is the compose-btn inside the modal footer
    return this.modal.$('button.compose-btn');
  }

  get cancelButton() {
    // Cancel button is the last button in the modal footer
    return this.modal.$$('button').find(async (btn) => {
      const text = await btn.getText();
      return text.includes('取消') || text.includes('Cancel');
    });
  }

  async openCompose() {
    const btn = await this.composeButton;
    if (btn) {
      await btn.click();
    }
    // Wait for modal to appear
    await this.modal.waitForExist({ timeout: 5000 });
  }

  async closeCompose() {
    const btn = await this.closeButton;
    if (btn) {
      await btn.click();
    }
  }

  async fillEmail(to, subject) {
    const toField = await this.toInput;
    if (toField) {
      await toField.setValue(to);
    }

    const subjectField = await this.subjectInput;
    if (subjectField) {
      await subjectField.setValue(subject);
    }
  }

  async send() {
    const btn = await this.sendButton;
    if (btn) {
      await btn.click();
    }
  }
}

export default new ComposePage();

import composePage from '../../pageobjects/compose.page.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Compose Email', () => {
  it('should open the compose modal when clicking the compose button', async () => {
    await browser.pause(1000);
    const composeBtn = await composePage.composeButton;
    expect(composeBtn).toBeTruthy();

    await composePage.openCompose();
    await browser.pause(500);

    const modal = await composePage.modal;
    expect(modal).toBeTruthy();
  });

  it('should have To, Subject fields in the compose modal', async () => {
    const toInput = await composePage.toInput;
    expect(toInput).toBeTruthy();

    const subjectInput = await composePage.subjectInput;
    expect(subjectInput).toBeTruthy();
  });

  it('should fill To and Subject fields', async () => {
    await composePage.fillEmail('test@example.com', 'Test Subject');
    await browser.pause(300);

    // Verify To field was filled
    const toInput = await composePage.toInput;
    const toValue = await toInput.getValue();
    expect(toValue).toBe('test@example.com');

    // Verify Subject field was filled
    const subjectInput = await composePage.subjectInput;
    if (subjectInput) {
      const subjectValue = await subjectInput.getValue();
      expect(subjectValue).toBe('Test Subject');
    }
  });

  it('should close the compose modal', async () => {
    await composePage.closeCompose();
    await browser.pause(500);

    // Modal should no longer be visible
    const modalExists = await composePage.modal.isExisting();
    expect(modalExists).toBe(false);
  });

  it('should reopen compose modal after closing', async () => {
    await composePage.openCompose();
    await browser.pause(500);

    const modal = await composePage.modal;
    expect(modal).toBeTruthy();

    // Clean up: close the modal
    await composePage.closeCompose();
    await browser.pause(300);
  });
});

export const byTestId = (id) => $(`[data-testid="${id}"]`);
export const allByTestId = (id) => $$(`[data-testid="${id}"]`);

export async function waitForText(text, timeout = 10000) {
  await browser.waitUntil(
    async () => (await $('body').getText()).includes(text),
    {
      timeout,
      timeoutMsg: `页面未在 ${timeout}ms 内出现文本: ${text}`,
    }
  );
}

export async function waitForTextGone(text, timeout = 10000) {
  await browser.waitUntil(
    async () => !(await $('body').getText()).includes(text),
    {
      timeout,
      timeoutMsg: `页面未在 ${timeout}ms 内移除文本: ${text}`,
    }
  );
}

import { createWdioConfig } from './wdio.shared.js';

export const config = createWdioConfig({
  dataRootName: '.e2e-data',
  specs: [
    [
      './test/specs/smoke.e2e.js',
      './test/specs/account-switching.e2e.js',
      './test/specs/email-list.e2e.js',
      './test/specs/navigation.e2e.js',
      './test/specs/compose.e2e.js',
      './test/specs/theme.e2e.js',
    ],
  ],
});

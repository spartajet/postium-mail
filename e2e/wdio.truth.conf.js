import path from 'path';
import { fileURLToPath } from 'url';
import { createWdioConfig } from './wdio.shared.js';
import {
  loadTruthAccounts,
  maskEmail,
  truthConfigPath,
} from './helpers/truth-config.js';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const truthAccounts = loadTruthAccounts(rootDir);

export const config = createWdioConfig({
  dataRootName: '.e2e-truth-data',
  mochaTimeout: 300000,
  specs: [
    [
      './test/specs/real-accounts.e2e.js',
    ],
  ],
  env: {
    POSTIUM_E2E_TRUTH: '1',
    POSTIUM_REAL_MAIL: '1',
  },
  snapshot: {
    truth: {
      enabled: true,
      configPath: truthConfigPath(rootDir),
      accountKeys: truthAccounts.map((account) => account.key),
      maskedEmails: truthAccounts.map((account) => maskEmail(account.email)),
    },
  },
});

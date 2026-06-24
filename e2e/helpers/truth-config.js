import fs from 'fs';
import path from 'path';

export function truthConfigPath(rootDir) {
  return process.env.POSTIUM_REAL_MAIL_CONFIG
    ? path.resolve(process.env.POSTIUM_REAL_MAIL_CONFIG)
    : path.resolve(rootDir, '.test_mail_accounts.json');
}

export function loadTruthAccounts(rootDir) {
  if (process.env.POSTIUM_REAL_MAIL !== '1') {
    throw new Error('truth E2E 必须通过 POSTIUM_REAL_MAIL=1 显式启用');
  }

  const configPath = truthConfigPath(rootDir);
  const raw = fs.readFileSync(configPath, 'utf8');
  const parsed = JSON.parse(raw);
  const entries = Object.entries(parsed.accounts ?? {});

  if (entries.length === 0) {
    throw new Error(`truth 配置 ${configPath} 必须包含至少一个 accounts 条目`);
  }

  return entries.map(([key, account]) => {
    validateAccount(key, account);
    return { key, ...account };
  });
}

export function truthRunId() {
  return `${new Date().toISOString().replace(/[-:.TZ]/g, '')}-${process.pid}`;
}

export function maskEmail(email) {
  const [local, domain] = String(email).split('@');
  if (!local || !domain) return '***';
  return `${local[0]}***@${domain}`;
}

export function selfSubject(runId, key) {
  return `[Postium Truth ${runId}] self ${key}`;
}

export function pairSubject(runId, fromKey, toKey) {
  return `[Postium Truth ${runId}] pair ${fromKey} to ${toKey}`;
}

function validateAccount(key, account) {
  if (!account?.email) throw new Error(`truth account ${key} 缺少 email`);
  if (!account?.password) throw new Error(`truth account ${key} 缺少 password`);
  validateServer(key, 'imap', account.imap);
  validateServer(key, 'smtp', account.smtp);
}

function validateServer(key, kind, server) {
  if (!server?.host) throw new Error(`truth account ${key} 缺少 ${kind}.host`);
  if (!Number(server?.port)) throw new Error(`truth account ${key} 缺少 ${kind}.port`);
  if (typeof server?.ssl !== 'boolean') {
    throw new Error(`truth account ${key} 缺少 ${kind}.ssl boolean`);
  }
}

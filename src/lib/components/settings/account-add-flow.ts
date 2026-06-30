import type { InitialSyncRange } from "$lib/bindings";

type ContinueAfterAccountAddedOptions = {
  accountId: number;
  loadAccounts: () => Promise<void>;
  setActive: (accountId: number) => void;
  close: () => void;
  goHome: () => Promise<void>;
};

export async function continueAfterAccountAdded({
  accountId,
  loadAccounts,
  setActive,
  close,
  goHome,
}: ContinueAfterAccountAddedOptions) {
  await loadAccounts();
  setActive(accountId);
  close();
  await goHome();

  return { readyForInitialSync: true };
}

type StartInitialSyncOptions = {
  accountId: number;
  range: InitialSyncRange;
  syncAccountWithRange: (
    accountId: number,
    range: InitialSyncRange,
  ) => Promise<void>;
};

export async function startInitialSyncAfterAccountAdded({
  accountId,
  range,
  syncAccountWithRange,
}: StartInitialSyncOptions) {
  await syncAccountWithRange(accountId, range);

  return { syncStarted: true };
}

type AccountEmailIdentity = {
  id: number;
  email: string;
};

type ResolveOAuth2CompletedAccountOptions = {
  completedEmail: string;
  loadAccounts: () => Promise<void>;
  getAccounts: () => AccountEmailIdentity[];
};

export async function resolveOAuth2CompletedAccount({
  completedEmail,
  loadAccounts,
  getAccounts,
}: ResolveOAuth2CompletedAccountOptions) {
  await loadAccounts();

  return (
    getAccounts().find((account) => account.email === completedEmail)?.id ??
    null
  );
}

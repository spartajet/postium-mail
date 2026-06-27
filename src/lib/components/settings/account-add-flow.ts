type ContinueAfterAccountAddedOptions = {
  accountId: number;
  loadAccounts: () => Promise<void>;
  setActive: (accountId: number) => void;
  close: () => void;
  goHome: () => Promise<void>;
  syncAccount: (accountId: number) => Promise<void>;
};

export async function continueAfterAccountAdded({
  accountId,
  loadAccounts,
  setActive,
  close,
  goHome,
  syncAccount,
}: ContinueAfterAccountAddedOptions) {
  await loadAccounts();
  setActive(accountId);
  close();
  await goHome();

  void syncAccount(accountId).catch(() => {
    // 同步状态和错误提示由全局同步 store 负责处理；这里不能阻塞主界面跳转。
  });

  return { syncStarted: true };
}

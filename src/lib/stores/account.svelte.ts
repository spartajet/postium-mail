import { getContext, setContext } from 'svelte';
import { commands } from '$lib/bindings';
import type { AccountDto } from '$lib/bindings';
import { formatError } from '$lib/utils/error.js';

class AccountState {
  accounts = $state<AccountDto[]>([]);
  activeAccountId = $state<number | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  activeAccount = $derived(
    this.accounts.find(a => a.id === this.activeAccountId) ?? null
  );

  async loadAccounts() {
    this.loading = true;
    this.error = null;
    try {
      const result = await commands.listAccounts();
      if (result.status === 'ok') {
        this.accounts = result.data;
        if (!this.activeAccountId && this.accounts.length > 0) {
          this.activeAccountId = this.accounts[0]!.id;
        }
      } else {
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      this.error = formatError(e);
    } finally {
      this.loading = false;
    }
  }

  setActive(id: number) {
    this.activeAccountId = id;
  }

  async deleteAccount(id: number) {
    try {
      const result = await commands.deleteAccount(id);
      if (result.status === 'ok') {
        this.accounts = this.accounts.filter(a => a.id !== id);
        if (this.activeAccountId === id) {
          this.activeAccountId = this.accounts.length > 0 ? this.accounts[0]!.id : null;
        }
      }
    } catch (e: unknown) {
      this.error = formatError(e);
    }
  }
}

const ACCOUNT_KEY = Symbol('account');

export function createAccountState() {
  const state = new AccountState();
  setContext(ACCOUNT_KEY, state);
  return state;
}

export function getAccountState() {
  return getContext<AccountState>(ACCOUNT_KEY);
}

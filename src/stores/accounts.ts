import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface Account {
  id: string;
  name: string;
  email: string;
  provider: string;
  imap_host: string;
  imap_port: number;
  smtp_host: string;
  smtp_port: number;
}

interface AccountStore {
  accounts: Account[];
  activeAccountId: string | null;
  loading: boolean;
  error: string | null;
  fetchAccounts: () => Promise<void>;
  setActiveAccount: (id: string) => void;
  removeAccount: (id: string) => Promise<void>;
}

export const useAccountStore = create<AccountStore>((set, get) => ({
  accounts: [],
  activeAccountId: null,
  loading: false,
  error: null,

  fetchAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const accounts = await invoke<Account[]>("list_accounts");
      set({
        accounts,
        loading: false,
        activeAccountId: accounts[0]?.id ?? null,
      });
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  setActiveAccount: (id) => set({ activeAccountId: id }),

  removeAccount: async (id) => {
    await invoke("remove_account", { accountId: id });
    const accounts = get().accounts.filter((a) => a.id !== id);
    set({
      accounts,
      activeAccountId: accounts[0]?.id ?? null,
    });
  },
}));

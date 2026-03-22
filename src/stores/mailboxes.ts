import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface Mailbox {
  id: string;
  account_id: string;
  name: string;
  delimiter: string | null;
  flags: string[];
  uid_validity: number | null;
  uid_next: number | null;
  thread_count: number;
  unread_count: number;
  folder_role: string | null;
}

interface MailboxStore {
  mailboxes: Mailbox[];
  selectedMailboxId: string | null;
  loading: boolean;
  error: string | null;
  fetchMailboxes: (accountId: string, refresh?: boolean) => Promise<Mailbox[]>;
  selectMailbox: (mailboxId: string) => void;
}

function pickDefaultMailbox(mailboxes: Mailbox[]) {
  return mailboxes.find((mailbox) => mailbox.name.toUpperCase() === "INBOX") ?? mailboxes[0] ?? null;
}

export const useMailboxStore = create<MailboxStore>((set, get) => ({
  mailboxes: [],
  selectedMailboxId: null,
  loading: false,
  error: null,

  fetchMailboxes: async (accountId, refresh = false) => {
    set({ loading: true, error: null });
    try {
      const mailboxes = await invoke<Mailbox[]>("list_mailboxes", {
        request: {
          account_id: accountId,
          refresh,
        },
      });
      const currentSelection = get().selectedMailboxId;
      const hasCurrentSelection = mailboxes.some((mailbox) => mailbox.id === currentSelection);
      const defaultMailbox = pickDefaultMailbox(mailboxes);

      set({
        mailboxes,
        selectedMailboxId: hasCurrentSelection ? currentSelection : defaultMailbox?.id ?? null,
        loading: false,
      });

      return mailboxes;
    } catch (e) {
      const error = String(e);
      set({ mailboxes: [], selectedMailboxId: null, loading: false, error });
      throw e;
    }
  },

  selectMailbox: (mailboxId) => set({ selectedMailboxId: mailboxId }),
}));

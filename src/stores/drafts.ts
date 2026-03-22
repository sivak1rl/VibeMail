import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface DraftData {
  id: string;
  account_id: string | null;
  mode: string;
  to_addrs: string;
  cc_addrs: string;
  bcc_addrs: string;
  subject: string;
  body_text: string;
  body_html: string | null;
  in_reply_to: string | null;
  thread_id: string | null;
  updated_at: number;
}

export interface SaveDraftPayload {
  account_id: string | null;
  mode: string;
  to_addrs: string;
  cc_addrs: string;
  bcc_addrs: string;
  subject: string;
  body_text: string;
  body_html: string | null;
  in_reply_to: string | null;
  thread_id: string | null;
}

interface DraftStore {
  drafts: DraftData[];
  draftCount: number;
  saveDraft: (id: string, payload: SaveDraftPayload) => Promise<void>;
  loadDraft: (id: string) => Promise<DraftData | null>;
  deleteDraft: (id: string) => Promise<void>;
  fetchDrafts: (accountId: string) => Promise<void>;
  fetchDraftCount: (accountId: string) => Promise<void>;
  syncDraftToImap: (id: string) => Promise<void>;
  deleteDraftFromImap: (id: string) => Promise<void>;
}

export const useDraftStore = create<DraftStore>((set) => ({
  drafts: [],
  draftCount: 0,

  saveDraft: async (id, payload) => {
    await invoke("save_draft", { request: { id, ...payload } });
  },

  loadDraft: async (id) => {
    return invoke<DraftData | null>("get_draft", { id });
  },

  deleteDraft: async (id) => {
    await invoke("delete_draft", { id });
    set((state) => ({
      drafts: state.drafts.filter((d) => d.id !== id),
      draftCount: Math.max(0, state.draftCount - 1),
    }));
  },

  fetchDrafts: async (accountId) => {
    const drafts = await invoke<DraftData[]>("list_drafts", { accountId });
    set({ drafts, draftCount: drafts.length });
  },

  fetchDraftCount: async (accountId) => {
    const count = await invoke<number>("count_drafts", { accountId });
    set({ draftCount: count });
  },

  syncDraftToImap: async (id) => {
    await invoke("sync_draft_to_imap", { id }).catch((e) => {
      console.warn("Failed to sync draft to IMAP:", e);
    });
  },

  deleteDraftFromImap: async (id) => {
    await invoke("delete_draft_from_imap", { id }).catch((e) => {
      console.warn("Failed to delete draft from IMAP:", e);
    });
  },
}));

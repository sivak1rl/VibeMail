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
  saveDraft: (id: string, payload: SaveDraftPayload) => Promise<void>;
  loadDraft: (id: string) => Promise<DraftData | null>;
  deleteDraft: (id: string) => Promise<void>;
}

export const useDraftStore = create<DraftStore>(() => ({
  saveDraft: async (id, payload) => {
    await invoke("save_draft", { request: { id, ...payload } });
  },
  loadDraft: async (id) => {
    return invoke<DraftData | null>("get_draft", { id });
  },
  deleteDraft: async (id) => {
    await invoke("delete_draft", { id });
  },
}));

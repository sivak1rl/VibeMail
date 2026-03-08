import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAiStore, type AiConfig } from "../stores/ai";
import { useAccountStore } from "../stores/accounts";
import {
  usePreferencesStore,
  type CustomCategoryPreference,
} from "../stores/preferences";
import styles from "./Settings.module.css";

interface Props {
  onBack: () => void;
  onReset: () => void;
}

const PROVIDERS = [
  { value: "ollama", label: "Local Ollama (default — no key needed)" },
  { value: "openai_compat", label: "BYOK: OpenAI-compatible (Claude, OpenAI, Mistral, Groq…)" },
];

export default function Settings({ onBack, onReset }: Props) {
  const { config, loadConfig, saveConfig } = useAiStore();
  const { accounts, removeAccount } = useAccountStore();
  const {
    autoSyncIntervalMinutes,
    setAutoSyncIntervalMinutes,
    autoLabelNewEmails,
    setAutoLabelNewEmails,
    showMessageDetailsByDefault,
    setShowMessageDetailsByDefault,
    historyFetchDays,
    setHistoryFetchDays,
    historyFetchLimit,
    setHistoryFetchLimit,
    customCategories,
    setCustomCategories,
    signatures,
    setSignature,
  } = usePreferencesStore();

  const [dbCounts, setDbCounts] = useState<Record<string, number>>({});
  const [form, setForm] = useState<AiConfig>({
    provider: "ollama",
    base_url: "http://localhost:11434",
    model_triage: "llama3.2:3b",
    model_summary: "llama3.1:8b",
    model_draft: "llama3.1:8b",
    model_extract: "llama3.2:3b",
    model_embed: "nomic-embed-text",
    privacy_mode: false,
    enabled: true,
  });

  useEffect(() => {
    loadConfig();
    const fetchCounts = async () => {
      try {
        const counts = await invoke<Record<string, number>>("get_db_counts");
        setDbCounts(counts);
      } catch {}
    };
    void fetchCounts();
  }, [loadConfig]);
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);
  const [customDraft, setCustomDraft] = useState<CustomCategoryPreference[]>([]);

  useEffect(() => {
    if (config) setForm(config);
  }, [config]);

  useEffect(() => {
    setCustomDraft(customCategories);
  }, [customCategories]);

  const handleSave = async () => {
    await saveConfig(form, apiKey || undefined);
    setCustomCategories(customDraft);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const field = (key: keyof AiConfig) => ({
    value: form[key] as string,
    onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
      setForm((f) => ({ ...f, [key]: e.target.value })),
  });

  const updateCategory = (
    index: number,
    update: (current: CustomCategoryPreference) => CustomCategoryPreference,
  ) => {
    setCustomDraft((current) =>
      current.map((item, idx) => (idx === index ? update(item) : item)),
    );
  };

  const scrollTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth" });
  };

  const handleWipeData = async () => {
    if (!window.confirm("Are you SURE you want to wipe all local email data? This will remove all emails and AI embeddings. Your account credentials will be kept, but automatic sync will be disabled.")) {
      return;
    }
    
    const fullFileWipe = window.confirm("Would you also like to reset the database schema? (Recommended if you are seeing 'ON CONFLICT' errors)");

    try {
      // 1. Disable auto-sync
      setAutoSyncIntervalMinutes(0);
      
      // 2. Wipe data (backend now preserves accounts)
      await invoke("wipe_local_data", { resetSchema: fullFileWipe });
      
      alert("Local email data has been cleared. Your account will not be removed but automatic sync will be disabled for this email account.");
      onReset();
    } catch (e) {
      alert("Failed to wipe data: " + e);
    }
  };

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>← Back</button>
        <h1 className={styles.title}>Settings</h1>
      </div>

      <div className={styles.container}>
        <aside className={styles.sidebar}>
          <button className={styles.navLink} onClick={() => scrollTo("accounts")}>Accounts</button>
          <button className={styles.navLink} onClick={() => scrollTo("signatures")}>Signatures</button>
          <button className={styles.navLink} onClick={() => scrollTo("ai")}>AI Provider</button>
          <button className={styles.navLink} onClick={() => scrollTo("privacy")}>Privacy</button>
          <button className={styles.navLink} onClick={() => scrollTo("sync")}>Sync</button>
          <button className={styles.navLink} onClick={() => scrollTo("categories")}>Custom Categories</button>
          <button className={styles.navLink} onClick={() => scrollTo("danger")}>Danger Zone</button>

          <div style={{ flex: 1 }} />
          <button className={styles.saveBtn} onClick={handleSave}>
            {saved ? "Saved ✓" : "Save Settings"}
          </button>
        </aside>

        <div className={styles.content}>
          {/* Accounts */}
          <section id="accounts" className={styles.section}>
            <h2 className={styles.sectionTitle}>Accounts</h2>
            {accounts.length === 0 && (
              <p className={styles.muted}>No accounts configured.</p>
            )}
            {accounts.map((acc) => (
              <div key={acc.id} className={styles.accountRow}>
                <div>
                  <div className={styles.accountEmail}>{acc.email}</div>
                  <div className={styles.accountProvider}>{acc.provider}</div>
                </div>
                <button
                  className={styles.removeBtn}
                  onClick={() => removeAccount(acc.id)}
                >
                  Remove
                </button>
              </div>
            ))}
          </section>

          {/* Signatures */}
          <section id="signatures" className={styles.section}>
            <h2 className={styles.sectionTitle}>Signatures</h2>
            <p className={styles.muted}>Appended automatically when composing new messages.</p>
            {accounts.length === 0 && <p className={styles.muted}>No accounts configured.</p>}
            {accounts.map((acc) => (
              <div key={acc.id} style={{ marginBottom: "16px" }}>
                <div className={styles.accountEmail} style={{ marginBottom: "6px" }}>{acc.email}</div>
                <textarea
                  className={styles.textarea}
                  rows={4}
                  placeholder="Your signature (optional)"
                  value={signatures[acc.id] ?? ""}
                  onChange={(e) => setSignature(acc.id, e.target.value)}
                />
              </div>
            ))}
          </section>

          {/* AI */}
          <section id="ai" className={styles.section}>
            <h2 className={styles.sectionTitle}>AI Provider</h2>

            <label className={styles.label}>
              Provider
              <select className={styles.select} value={form.provider} onChange={(e) => setForm((f) => ({ ...f, provider: e.target.value }))}>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value}>{p.label}</option>
                ))}
              </select>
            </label>

            <label className={styles.label}>
              Base URL
              <input className={styles.input} type="text" {...field("base_url")} />
            </label>

            {form.provider === "openai_compat" && (
              <label className={styles.label}>
                API Key
                <input
                  className={styles.input}
                  type="password"
                  placeholder="sk-... (stored in OS keychain)"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </label>
            )}

            <h3 className={styles.subTitle}>Model Assignments</h3>

            <label className={styles.label}>
              Triage / Fast tasks
              <input className={styles.input} type="text" {...field("model_triage")} />
            </label>
            <label className={styles.label}>
              Summarization
              <input className={styles.input} type="text" {...field("model_summary")} />
            </label>
            <label className={styles.label}>
              Reply drafting
              <input className={styles.input} type="text" {...field("model_draft")} />
            </label>
            <label className={styles.label}>
              Action extraction
              <input className={styles.input} type="text" {...field("model_extract")} />
            </label>
            <label className={styles.label}>
              Embeddings
              <input className={styles.input} type="text" {...field("model_embed")} />
            </label>
          </section>

          {/* Privacy */}
          <section id="privacy" className={styles.section}>
            <h2 className={styles.sectionTitle}>Privacy</h2>

            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={form.privacy_mode}
                onChange={(e) => setForm((f) => ({ ...f, privacy_mode: e.target.checked }))}
              />
              Privacy mode — strip sender/recipient names before sending to AI API
            </label>

            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={form.enabled}
                onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
              />
              Enable AI features
            </label>
          </section>

          <section id="sync" className={styles.section}>
            <h2 className={styles.sectionTitle}>Sync</h2>
            <label className={styles.label}>
              Auto-sync interval (minutes)
              <input
                className={styles.input}
                type="number"
                min={0}
                step={1}
                value={autoSyncIntervalMinutes}
                onChange={(e) => setAutoSyncIntervalMinutes(Number(e.target.value))}
              />
            </label>
            <p className={styles.muted}>Use 0 to disable background auto-sync.</p>

            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={autoLabelNewEmails}
                onChange={(e) => setAutoLabelNewEmails(e.target.checked)}
              />
              Automatically apply category labels after sync (new unread threads)
            </label>

            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={showMessageDetailsByDefault}
                onChange={(e) => setShowMessageDetailsByDefault(e.target.checked)}
              />
              Show message header details (From, To, Cc, Date) by default when opening a message
            </label>

            <label className={styles.label} style={{ marginTop: "16px" }}>
              History fetch duration (days)
              <input
                className={styles.input}
                type="number"
                min={1}
                step={1}
                value={historyFetchDays}
                onChange={(e) => setHistoryFetchDays(Number(e.target.value))}
              />
            </label>
            <p className={styles.muted}>How many days of older mail to fetch when clicking "Load older emails".</p>

            <label className={styles.label} style={{ marginTop: "16px" }}>
              Max emails to download per history fetch
              <input
                className={styles.input}
                type="number"
                min={1}
                step={50}
                value={historyFetchLimit}
                onChange={(e) => setHistoryFetchLimit(Number(e.target.value))}
              />
            </label>
            <p className={styles.muted}>Limit the number of older messages downloaded in one go.</p>

            <div className={styles.statsGrid}>
              <div className={styles.statItem}>
                <span className={styles.statValue}>{dbCounts.threads || 0}</span>
                <span className={styles.statLabel}>Threads</span>
              </div>
              <div className={styles.statItem}>
                <span className={styles.statValue}>{dbCounts.messages || 0}</span>
                <span className={styles.statLabel}>Messages</span>
              </div>
              <div className={styles.statItem}>
                <span className={styles.statValue}>{dbCounts.attachments || 0}</span>
                <span className={styles.statLabel}>Attachments</span>
              </div>
            </div>
          </section>

          <section id="categories" className={styles.section}>
            <h2 className={styles.sectionTitle}>Custom Categories</h2>
            <p className={styles.muted}>
              Optional categories used during labeling. Name must be unique.
            </p>
            {customDraft.map((category, index) => (
              <div key={`${index}-${category.name}`} className={styles.categoryCard}>
                <label className={styles.label}>
                  Category name
                  <input
                    className={styles.input}
                    type="text"
                    maxLength={32}
                    value={category.name}
                    onChange={(e) =>
                      updateCategory(index, (current) => ({
                        ...current,
                        name: e.target.value,
                      }))
                    }
                  />
                </label>
                <label className={styles.label}>
                  Examples (one per line)
                  <textarea
                    className={styles.textarea}
                    rows={4}
                    value={category.examples.join("\n")}
                    onChange={(e) =>
                      updateCategory(index, (current) => ({
                        ...current,
                        examples: e.target.value
                          .split("\n")
                          .map((line) => line.trim())
                          .filter(Boolean)
                          .slice(0, 6),
                      }))
                    }
                  />
                </label>
                <button
                  className={styles.removeBtn}
                  onClick={() =>
                    setCustomDraft((current) =>
                      current.filter((_, idx) => idx !== index),
                    )
                  }
                >
                  Remove Category
                </button>
              </div>
            ))}
            <button
              className={styles.addBtn}
              onClick={() =>
                setCustomDraft((current) =>
                  current.length >= 12
                    ? current
                    : [...current, { name: "", examples: [] }],
                )
              }
            >
              Add Custom Category
            </button>
          </section>

          {/* Danger Zone */}
          <section id="danger" className={styles.section}>
            <h2 className={`${styles.sectionTitle} ${styles.dangerTitle}`}>Danger Zone</h2>
            <div className={styles.dangerCard}>
              <p className={styles.dangerText}>
                Wiping local data will permanently delete all email content, metadata, 
                and AI search indices from this machine. Your account login will be preserved,
                but automatic syncing will be disabled.
              </p>
              <button className={styles.wipeBtn} onClick={handleWipeData}>
                Wipe All Local Data
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

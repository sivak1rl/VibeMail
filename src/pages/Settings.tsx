import { useEffect, useState } from "react";
import { useAiStore, type AiConfig } from "../stores/ai";
import { useAccountStore } from "../stores/accounts";
import {
  usePreferencesStore,
  type CustomCategoryPreference,
} from "../stores/preferences";
import styles from "./Settings.module.css";

interface Props {
  onBack: () => void;
}

const PROVIDERS = [
  { value: "ollama", label: "Local Ollama (default — no key needed)" },
  { value: "openai_compat", label: "BYOK: OpenAI-compatible (Claude, OpenAI, Mistral, Groq…)" },
];

export default function Settings({ onBack }: Props) {
  const { config, loadConfig, saveConfig } = useAiStore();
  const { accounts, removeAccount } = useAccountStore();
  const {
    autoSyncIntervalMinutes,
    setAutoSyncIntervalMinutes,
    autoLabelNewEmails,
    setAutoLabelNewEmails,
    customCategories,
    setCustomCategories,
  } = usePreferencesStore();

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
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);
  const [customDraft, setCustomDraft] = useState<CustomCategoryPreference[]>([]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

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

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>← Back</button>
        <h1 className={styles.title}>Settings</h1>
      </div>

      <div className={styles.content}>
        {/* Accounts */}
        <section className={styles.section}>
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

        {/* AI */}
        <section className={styles.section}>
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
        <section className={styles.section}>
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

        <section className={styles.section}>
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
        </section>

        <section className={styles.section}>
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

        <button className={styles.saveBtn} onClick={handleSave}>
          {saved ? "Saved ✓" : "Save Settings"}
        </button>
      </div>
    </div>
  );
}

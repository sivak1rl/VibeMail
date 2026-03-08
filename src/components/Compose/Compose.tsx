import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Message, Thread } from "../../stores/threads";
import { useAccountStore } from "../../stores/accounts";
import { useAiStore } from "../../stores/ai";
import styles from "./Compose.module.css";

interface Props {
  thread?: Thread;
  messages?: Message[];
  onClose: () => void;
  expanded?: boolean;
  onExpandChange?: (expanded: boolean) => void;
}

const NEW_KEY = "__new__";

export default function Compose({ thread, messages = [], onClose, expanded = false, onExpandChange }: Props) {
  const { activeAccountId } = useAccountStore();
  const { draftByThread, draftStreaming, draftReply, draftNew, config } = useAiStore();

  const isReply = !!thread;
  const lastMsg = messages[messages.length - 1];

  const [to, setTo] = useState(lastMsg?.from[0]?.email ?? "");
  const [subject, setSubject] = useState(
    isReply
      ? (thread.subject?.startsWith("Re:") ? thread.subject : `Re: ${thread.subject ?? ""}`)
      : ""
  );
  const [body, setBody] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);

  const draftKey = isReply ? thread.id : NEW_KEY;
  const draft = draftByThread[draftKey];
  const isGenerating = draftStreaming[draftKey];

  // When draft is ready, populate the body
  useEffect(() => {
    if (draft && !isGenerating && !body) {
      setBody(draft);
    }
  }, [draft, isGenerating]);

  const handleReplyAIDraft = async () => {
    if (!thread) return;
    setError(null);
    try {
      await draftReply(thread.id);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleNewAIDraft = async () => {
    if (!aiPrompt.trim()) return;
    setAiError(null);
    try {
      const result = await draftNew(aiPrompt.trim());
      if (result) setBody(result);
    } catch (e) {
      setAiError(String(e));
    }
  };

  const handleSend = async () => {
    if (!activeAccountId || !body.trim() || !to.trim()) return;
    setSending(true);
    setError(null);
    try {
      await invoke("send_message", {
        message: {
          account_id: activeAccountId,
          to: [{ name: null, email: to }],
          cc: null,
          bcc: null,
          subject,
          body_text: body,
          body_html: null,
          in_reply_to: lastMsg?.message_id ?? null,
          references: lastMsg ? [lastMsg.message_id].filter(Boolean) : null,
        },
      });
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <div className={styles.compose}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>{isReply ? "Reply" : "New Message"}</span>
        <div className={styles.headerActions}>
          <button
            className={styles.expandBtn}
            onClick={() => onExpandChange?.(!expanded)}
            title={expanded ? "Collapse" : "Expand"}
          >
            {expanded ? "⤫" : "⤢"}
          </button>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>
      </div>

      <div className={styles.body}>
        {/* Compose form */}
        <div className={styles.form}>
          <div className={styles.meta}>
            <span className={styles.metaLabel}>To:</span>
            {isReply ? (
              <span className={styles.metaValue}>{to}</span>
            ) : (
              <input
                className={styles.metaInput}
                value={to}
                onChange={(e) => setTo(e.target.value)}
                placeholder="recipient@example.com"
                autoFocus={!expanded}
              />
            )}
          </div>
          <div className={styles.meta}>
            <span className={styles.metaLabel}>Subject:</span>
            {isReply ? (
              <span className={styles.metaValue}>{subject}</span>
            ) : (
              <input
                className={styles.metaInput}
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
                placeholder="Subject"
              />
            )}
          </div>

          <div className={styles.bodyArea}>
            {isGenerating && (
              <div className={styles.generating}>
                <span className={styles.cursor}>▌</span> Drafting with AI...
              </div>
            )}
            <textarea
              className={styles.textarea}
              value={isGenerating ? (draft ?? "") : body}
              onChange={(e) => setBody(e.target.value)}
              disabled={isGenerating || sending}
              placeholder={isReply ? "Write your reply..." : "Write your message..."}
            />
          </div>

          {error && <div className={styles.error}>{error}</div>}

          <div className={styles.footer}>
            {isReply && (
              <button
                className={styles.aiBtn}
                onClick={handleReplyAIDraft}
                disabled={isGenerating || sending}
              >
                {isGenerating ? "Generating..." : "✦ Draft with AI"}
              </button>
            )}
            <div className={styles.footerRight}>
              <button className={styles.cancelBtn} onClick={onClose} disabled={sending}>
                Cancel
              </button>
              <button
                className={styles.sendBtn}
                onClick={handleSend}
                disabled={sending || !body.trim() || !to.trim()}
              >
                {sending ? "Sending..." : "Send"}
              </button>
            </div>
          </div>
        </div>

        {/* AI assistant panel — only for new compose when expanded or AI enabled */}
        {!isReply && config?.enabled && (
          <div className={styles.aiPanel}>
            <div className={styles.aiPanelHeader}>✦ AI Assistant</div>
            <p className={styles.aiPanelHint}>
              Describe what you want to write and let AI draft it for you.
            </p>
            <textarea
              className={styles.aiPrompt}
              value={aiPrompt}
              onChange={(e) => setAiPrompt(e.target.value)}
              placeholder="e.g. Write to Sarah asking for a meeting next week to discuss the Q4 roadmap"
              rows={5}
              autoFocus={expanded}
            />
            {aiError && <div className={styles.error}>{aiError}</div>}
            <button
              className={styles.aiGenerateBtn}
              onClick={handleNewAIDraft}
              disabled={!aiPrompt.trim() || !!isGenerating}
            >
              {isGenerating ? "Generating..." : "✦ Generate Draft"}
            </button>
            {isGenerating && (
              <div className={styles.aiStream}>
                <span className={styles.cursor}>▌</span>
                {draftByThread[NEW_KEY] ?? ""}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Message, Thread } from "../../stores/threads";
import { useAccountStore } from "../../stores/accounts";
import { useAiStore } from "../../stores/ai";
import styles from "./Compose.module.css";

interface Props {
  thread: Thread;
  messages: Message[];
  onClose: () => void;
}

export default function Compose({ thread, messages, onClose }: Props) {
  const { activeAccountId } = useAccountStore();
  const { draftByThread, draftStreaming, draftReply } = useAiStore();

  const [body, setBody] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const lastMsg = messages[messages.length - 1];
  const replyTo = lastMsg?.from[0]?.email ?? "";
  const subject = thread.subject?.startsWith("Re:")
    ? thread.subject
    : `Re: ${thread.subject ?? ""}`;

  const draft = draftByThread[thread.id];
  const isGenerating = draftStreaming[thread.id];

  // When draft is ready, populate the body
  useEffect(() => {
    if (draft && !isGenerating && !body) {
      setBody(draft);
    }
  }, [draft, isGenerating]);

  const handleAIDraft = async () => {
    setError(null);
    try {
      await draftReply(thread.id);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSend = async () => {
    if (!activeAccountId || !body.trim()) return;
    setSending(true);
    setError(null);
    try {
      await invoke("send_message", {
        message: {
          account_id: activeAccountId,
          to: [{ name: null, email: replyTo }],
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
        <span className={styles.headerTitle}>Reply</span>
        <button className={styles.closeBtn} onClick={onClose}>✕</button>
      </div>

      <div className={styles.meta}>
        <span className={styles.metaLabel}>To:</span>
        <span className={styles.metaValue}>{replyTo}</span>
      </div>
      <div className={styles.meta}>
        <span className={styles.metaLabel}>Subject:</span>
        <span className={styles.metaValue}>{subject}</span>
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
          placeholder="Write your reply..."
          rows={12}
        />
      </div>

      {error && <div className={styles.error}>{error}</div>}

      <div className={styles.footer}>
        <button
          className={styles.aiBtn}
          onClick={handleAIDraft}
          disabled={isGenerating || sending}
        >
          {isGenerating ? "Generating..." : "✦ Draft with AI"}
        </button>
        <div className={styles.footerRight}>
          <button className={styles.cancelBtn} onClick={onClose} disabled={sending}>
            Cancel
          </button>
          <button
            className={styles.sendBtn}
            onClick={handleSend}
            disabled={sending || !body.trim()}
          >
            {sending ? "Sending..." : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}

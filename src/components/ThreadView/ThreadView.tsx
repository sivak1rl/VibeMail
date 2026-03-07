import { useEffect, useState } from "react";
import DOMPurify from "dompurify";
import { formatDistanceToNow } from "date-fns";
import type { Message, Thread } from "../../stores/threads";
import { useAiStore } from "../../stores/ai";
import AIPanel from "../AIPanel/AIPanel";
import Compose from "../Compose/Compose";
import styles from "./ThreadView.module.css";

interface Props {
  thread: Thread | null;
  messages: Message[];
}

function MessageItem({ msg, defaultOpen }: { msg: Message; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const from = msg.from[0];
  const displayName = from?.name ?? from?.email ?? "Unknown";
  const dateStr = msg.date
    ? formatDistanceToNow(new Date(msg.date), { addSuffix: true })
    : "";

  return (
    <div className={`${styles.message} ${open ? styles.messageOpen : ""}`}>
      <div className={styles.messageHeader} onClick={() => setOpen((v) => !v)}>
        <div className={styles.msgMeta}>
          <span className={styles.fromName}>{displayName}</span>
          <span className={styles.msgDate}>{dateStr}</span>
        </div>
        <span className={styles.chevron}>{open ? "▾" : "▸"}</span>
      </div>
      {open && (
        <div className={styles.messageBody}>
          {msg.body_html ? (
            <iframe
              srcDoc={DOMPurify.sanitize(msg.body_html, { WHOLE_DOCUMENT: false })}
              sandbox="allow-same-origin"
              className={styles.htmlFrame}
              title="Email content"
            />
          ) : (
            <pre className={styles.textBody}>{msg.body_text ?? "[No content]"}</pre>
          )}
        </div>
      )}
    </div>
  );
}

export default function ThreadView({ thread, messages }: Props) {
  const [showCompose, setShowCompose] = useState(false);
  const { actionsByThread, loadThreadInsights } = useAiStore();

  useEffect(() => {
    if (!thread) return;
    void loadThreadInsights(thread.id);
  }, [thread, loadThreadInsights]);

  if (!thread) {
    return (
      <div className={styles.empty}>
        <p>Select a thread to read</p>
      </div>
    );
  }

  const actions = actionsByThread[thread.id] ?? [];

  return (
    <div className={styles.wrapper}>
      <div className={styles.main}>
        <div className={styles.threadHeader}>
          <h2 className={styles.subject}>{thread.subject ?? "(no subject)"}</h2>
          <div className={styles.headerActions}>
            <button
              className={styles.replyBtn}
              onClick={() => setShowCompose(true)}
            >
              Reply
            </button>
          </div>
        </div>

        {actions.length > 0 && (
          <div className={styles.actionsBar}>
            <span className={styles.actionsLabel}>Actions:</span>
            {actions.map((a, i) => (
              <span
                key={i}
                className={`${styles.actionChip} ${
                  a.priority === "high" ? styles.chipHigh : ""
                }`}
                title={a.date ?? undefined}
              >
                {a.kind === "date" ? "📅" : a.kind === "followup" ? "🔔" : "✓"}{" "}
                {a.text}
              </span>
            ))}
          </div>
        )}

        <div className={styles.messages}>
          {messages.map((msg, i) => (
            <MessageItem
              key={msg.id}
              msg={msg}
              defaultOpen={i === messages.length - 1}
            />
          ))}
        </div>

        {showCompose && (
          <Compose
            thread={thread}
            messages={messages}
            onClose={() => setShowCompose(false)}
          />
        )}
      </div>

      <AIPanel thread={thread} messages={messages} />
    </div>
  );
}

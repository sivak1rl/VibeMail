import { useEffect, useState, useMemo } from "react";
import DOMPurify from "dompurify";
import { formatDistanceToNow } from "date-fns";
import { invoke } from "@tauri-apps/api/core";
import type { Message, Thread } from "../../stores/threads";
import { useThreadStore } from "../../stores/threads";
import { useAiStore } from "../../stores/ai";
import AIPanel from "../AIPanel/AIPanel";
import Compose from "../Compose/Compose";
import styles from "./ThreadView.module.css";

interface Props {
  thread: Thread | null;
  messages: Message[];
}

interface AttachmentMetadata {
  id: string;
  filename: string;
  content_type: string;
  size: number;
}

function AttachmentItem({ att }: { att: AttachmentMetadata }) {
  const [loading, setLoading] = useState(false);

  const handleOpen = async () => {
    setLoading(true);
    try {
      await invoke("open_attachment", { id: att.id });
    } catch (e) {
      console.error("Failed to open attachment:", e);
    } finally {
      setLoading(false);
    }
  };

  const sizeKb = Math.round(att.size / 1024);

  return (
    <div className={styles.attachment} onClick={handleOpen}>
      <span className={styles.attIcon}>📄</span>
      <div className={styles.attInfo}>
        <span className={styles.attName}>{att.filename}</span>
        <span className={styles.attSize}>{sizeKb} KB</span>
      </div>
      {loading && <span className={styles.attSpinner}>⟳</span>}
    </div>
  );
}

function MessageItem({ msg, defaultOpen }: { msg: Message; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const [attachments, setAttachments] = useState<AttachmentMetadata[]>([]);
  const from = msg.from[0];
  const displayName = from?.name ?? from?.email ?? "Unknown";
  const dateStr = msg.date
    ? formatDistanceToNow(new Date(msg.date), { addSuffix: true })
    : "";

  useEffect(() => {
    if (msg.has_attachments) {
      void (async () => {
        try {
          const meta = await invoke<AttachmentMetadata[]>("list_attachments", {
            messageId: msg.id,
          });
          setAttachments(meta);
        } catch (e) {
          console.error("Failed to list attachments:", e);
        }
      })();
    }
  }, [msg.id, msg.has_attachments]);

  const sanitizedHtml = useMemo(() => {
    if (!msg.body_html) return "";
    return DOMPurify.sanitize(msg.body_html, {
      WHOLE_DOCUMENT: false,
      ADD_TAGS: ["img"],
      ADD_ATTR: ["src", "cid"],
    });
  }, [msg.body_html]);

  return (
    <div className={`${styles.message} ${open ? styles.messageOpen : ""}`}>
      <div className={styles.messageHeader} onClick={() => setOpen((v) => !v)}>
        <div className={styles.msgMeta}>
          <span className={styles.fromName}>{displayName}</span>
          <span className={styles.msgDate}>{dateStr}</span>
        </div>
        <div className={styles.msgHeaderRight}>
          {msg.has_attachments && <span className={styles.headerAttIcon}>📎</span>}
          <span className={styles.chevron}>{open ? "▾" : "▸"}</span>
        </div>
      </div>
      {open && (
        <div className={styles.messageBody}>
          {msg.body_html ? (
            <iframe
              srcDoc={sanitizedHtml}
              sandbox="allow-same-origin"
              className={styles.htmlFrame}
              title="Email content"
              onLoad={(e) => {
                const frame = e.currentTarget;
                if (frame.contentDocument) {
                  frame.style.height = `${frame.contentDocument.body.scrollHeight + 20}px`;
                }
              }}
            />
          ) : (
            <pre className={styles.textBody}>{msg.body_text ?? "[No content]"}</pre>
          )}

          {attachments.length > 0 && (
            <div className={styles.attachmentList}>
              {attachments.map((att) => (
                <AttachmentItem key={att.id} att={att} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function ThreadView({ thread, messages }: Props) {
  const [showCompose, setShowCompose] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const { setThreadsRead, setThreadsFlagged, archiveThreads } = useThreadStore();
  const { actionsByThread, loadThreadInsights } = useAiStore();

  const isUnread = useMemo(() => {
    return messages.some((m) => !m.flags.includes("\\Seen"));
  }, [messages]);

  const isStarred = useMemo(() => {
    return messages.some((m) => m.flags.includes("\\Flagged"));
  }, [messages]);

  useEffect(() => {
    if (!thread) return;
    void loadThreadInsights(thread.id);
    setIsExpanded(false); // Reset expansion when switching threads
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
    <div
      className={`${styles.wrapper} ${isExpanded ? styles.expanded : ""}`}
      onClick={() => isExpanded && setIsExpanded(false)}
    >
      <div
        className={styles.main}
        onClick={(e) => isExpanded && e.stopPropagation()}
      >
        <div className={styles.toolbar}>
          <button
            className={styles.toolbarBtn}
            onClick={() => archiveThreads([thread.id])}
            title="Archive"
          >
            📥 Archive
          </button>
          <button
            className={styles.toolbarBtn}
            onClick={() => setThreadsRead([thread.id], isUnread)}
            title={isUnread ? "Mark as read" : "Mark as unread"}
          >
            {isUnread ? "✉ Mark Read" : "📩 Mark Unread"}
          </button>
          <button
            className={`${styles.toolbarBtn} ${isStarred ? styles.toolbarBtnStarActive : ""}`}
            onClick={() => setThreadsFlagged([thread.id], !isStarred)}
            title={isStarred ? "Unstar" : "Star"}
          >
            {isStarred ? "★ Starred" : "☆ Star"}
          </button>
        </div>

        <div className={styles.threadHeader}>
          <h2 className={styles.subject}>{thread.subject ?? "(no subject)"}</h2>
          <div className={styles.headerActions}>
            <button
              className={styles.expandBtn}
              onClick={() => setIsExpanded(!isExpanded)}
              title={isExpanded ? "Reduce view" : "Expand to full screen"}
            >
              {isExpanded ? "⤫ Reduce" : "⤢ Expand"}
            </button>
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

      <div
        className={isExpanded ? styles.aiPanelWrapper : ""}
        onClick={(e) => isExpanded && e.stopPropagation()}
      >
        <AIPanel thread={thread} messages={messages} />
      </div>
    </div>
  );
}

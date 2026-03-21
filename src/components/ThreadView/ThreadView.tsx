import { useEffect, useState, useMemo } from "react";
import DOMPurify from "dompurify";
import { formatDistanceToNow, format } from "date-fns";
import { invoke } from "@tauri-apps/api/core";
import type { Message, Thread } from "../../stores/threads";
import { useThreadStore } from "../../stores/threads";
import { useAiStore } from "../../stores/ai";
import AIPanel from "../AIPanel/AIPanel";
import Compose, { type ComposeMode } from "../Compose/Compose";
import { usePreferencesStore } from "../../stores/preferences";
import styles from "./ThreadView.module.css";

interface Props {
  thread: Thread | null;
  messages: Message[];
  composeOpen: boolean;
  composeMode?: ComposeMode;
  onComposeClose: () => void;
  onReplyClick: (mode: ComposeMode) => void;
}

interface AttachmentMetadata {
  id: string;
  filename: string;
  content_type: string;
  size: number;
}

function AttachmentItem({ att }: { att: AttachmentMetadata }) {
  const [loading, setLoading] = useState(false);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    let localUrl: string | null = null;

    const isImage = att.content_type?.toLowerCase().startsWith("image/");
    if (isImage) {
      void (async () => {
        try {
          const data = await invoke<number[]>("get_attachment_data", { id: att.id });
          if (!mounted) return;
          const blob = new Blob([new Uint8Array(data)], { type: att.content_type || "image/png" });
          localUrl = URL.createObjectURL(blob);
          setPreviewUrl(localUrl);
        } catch (e) {
          if (mounted) console.error("AttachmentItem: Preview failed", e);
        }
      })();
    }

    return () => {
      mounted = false;
      const urlToRevoke = localUrl;
      if (urlToRevoke) {
        setTimeout(() => {
          try {
            if (typeof URL !== 'undefined' && URL.revokeObjectURL) {
              URL.revokeObjectURL(urlToRevoke);
            }
          } catch {}
        }, 0);
      }
    };
  }, [att.id, att.content_type]);

  const handleOpen = async (e: React.MouseEvent) => {
    e.stopPropagation();
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
      {previewUrl ? (
        <img src={previewUrl} className={styles.attPreview} alt="" />
      ) : (
        <span className={styles.attIcon}>📄</span>
      )}
      <div className={styles.attInfo}>
        <span className={styles.attName}>{att.filename || "unnamed"}</span>
        <span className={styles.attSize}>{sizeKb} KB</span>
      </div>
      {loading && <span className={styles.attSpinner}>⟳</span>}
    </div>
  );
}

function MessagePreviews({ attachments }: { attachments: AttachmentMetadata[] }) {
  const [previews, setPreviews] = useState<{ id: string; url: string }[]>([]);

  useEffect(() => {
    let mounted = true;
    const localUrls: string[] = [];

    const load = async () => {
      if (!Array.isArray(attachments) || attachments.length === 0) {
        if (mounted) setPreviews([]);
        return;
      }

      const images = attachments.filter((a) => a.content_type?.toLowerCase().startsWith("image/"));
      if (images.length === 0) {
        if (mounted) setPreviews([]);
        return;
      }

      const results: { id: string; url: string }[] = [];
      for (const img of images) {
        try {
          const data = await invoke<number[]>("get_attachment_data", { id: img.id });
          if (!mounted) break;
          if (data && data.length > 0) {
            const blob = new Blob([new Uint8Array(data)], { type: img.content_type || "image/png" });
            const url = URL.createObjectURL(blob);
            localUrls.push(url);
            results.push({ id: img.id, url });
          }
        } catch (err) {
          if (mounted) console.error("MessagePreviews: Failed", err);
        }
      }
      
      if (mounted) {
        setPreviews(results);
      }
    };

    void load();

    return () => {
      mounted = false;
      const urlsToRevoke = [...localUrls];
      setTimeout(() => {
        urlsToRevoke.forEach((url) => {
          try {
            if (typeof URL !== 'undefined' && URL.revokeObjectURL) {
               URL.revokeObjectURL(url);
            }
          } catch {
            // Ignore cleanup errors
          }
        });
      }, 0);
    };
  }, [attachments]);

  if (!Array.isArray(previews) || previews.length === 0) return null;

  return (
    <div className={styles.previewsPane} onClick={(e) => e.stopPropagation()}>
      {previews.map((p) => (
        <div key={p.id} className={styles.previewFrame}>
          <img src={p.url} className={styles.previewImg} alt="Preview" />
        </div>
      ))}
    </div>
  );
}

function formatAddrs(addrs: { name?: string | null; email: string }[]): string {
  return addrs.map(a => a.name ? `${a.name} <${a.email}>` : a.email).join(", ");
}

function MessageItem({ msg, defaultOpen }: { msg: Message; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const { showMessageDetailsByDefault } = usePreferencesStore();
  const [showDetails, setShowDetails] = useState(showMessageDetailsByDefault);
  const [attachments, setAttachments] = useState<AttachmentMetadata[]>([]);
  const from = msg.from[0];
  const displayName = from?.name ?? from?.email ?? "Unknown";
  const dateStr = msg.date
    ? formatDistanceToNow(new Date(msg.date), { addSuffix: true })
    : "";
  const fullDateStr = msg.date
    ? format(new Date(msg.date), "PPPp")
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
    const clean = DOMPurify.sanitize(msg.body_html, {
      WHOLE_DOCUMENT: true,
      ADD_TAGS: ["style", "link", "img"],
      ADD_ATTR: ["src", "cid", "style", "class", "id", "width", "height", "align", "valign",
                 "bgcolor", "color", "border", "cellpadding", "cellspacing", "colspan", "rowspan"],
      FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "input", "textarea", "button"],
      FORBID_ATTR: ["onerror", "onload", "onclick", "onmouseover"],
    });
    // Move href → data-href so links can't navigate the iframe.
    // The click handler in onLoad reads data-href and opens externally.
    const doc = new DOMParser().parseFromString(clean, "text/html");
    for (const a of doc.querySelectorAll("a[href]")) {
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("http")) {
        a.setAttribute("data-href", href);
        a.removeAttribute("href");
        (a as HTMLElement).style.cursor = "pointer";
        (a as HTMLElement).style.textDecoration = "underline";
      }
    }
    return doc.documentElement.outerHTML;
  }, [msg.body_html]);

  return (
    <div className={`${styles.message} ${open ? styles.messageOpen : ""}`}>
      <div className={styles.messageHeader} onClick={() => setOpen((v) => !v)}>
        <div className={styles.msgMeta}>
          <span className={styles.fromName}>{displayName}</span>
          <span className={styles.msgDate}>{dateStr}</span>
        </div>
        <div className={styles.msgHeaderRight}>
          {open && (
            <button
              className={styles.detailToggle}
              onClick={(e) => { e.stopPropagation(); setShowDetails((v) => !v); }}
              title={showDetails ? "Hide details" : "Show details"}
            >
              {showDetails ? "Details ▴" : "Details ▾"}
            </button>
          )}
          {msg.has_attachments && <span className={styles.headerAttIcon}>📎</span>}
          <span className={styles.chevron}>{open ? "▾" : "▸"}</span>
        </div>
      </div>
      {open && showDetails && (
        <div className={styles.msgDetailHeader}>
          <div className={styles.detailRow}>
            <span className={styles.detailLabel}>From</span>
            <span className={styles.detailValue}>{formatAddrs(msg.from)}</span>
          </div>
          <div className={styles.detailRow}>
            <span className={styles.detailLabel}>To</span>
            <span className={styles.detailValue}>{formatAddrs(msg.to)}</span>
          </div>
          {msg.cc.length > 0 && (
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Cc</span>
              <span className={styles.detailValue}>{formatAddrs(msg.cc)}</span>
            </div>
          )}
          {fullDateStr && (
            <div className={styles.detailRow}>
              <span className={styles.detailLabel}>Date</span>
              <span className={styles.detailValue}>{fullDateStr}</span>
            </div>
          )}
        </div>
      )}
      {open && (
        <div className={styles.messageBody}>
          <MessagePreviews attachments={attachments} />
          {msg.body_html ? (
            <iframe
              srcDoc={sanitizedHtml}
              sandbox="allow-same-origin allow-scripts"
              className={styles.htmlFrame}
              title="Email content"
              onLoad={(e) => {
                const frame = e.currentTarget;
                const doc = frame.contentDocument;
                if (!doc) return;
                frame.style.height = `${doc.body.scrollHeight + 20}px`;
                // Open links in system browser via data-href
                doc.addEventListener("click", (ev) => {
                  const anchor = (ev.target as HTMLElement).closest("a");
                  const href = anchor?.getAttribute("data-href");
                  if (href) {
                    ev.preventDefault();
                    invoke("open_url", { url: href }).catch(() => {});
                  }
                });
              }}
            />
          ) : (
            <pre className={styles.textBody}>{msg.body_text ?? "[No content]"}</pre>
          )}

          {attachments.length > 0 && (
            <div className={styles.attachmentList}>
              <div className={styles.attachmentHeader}>Attachments ({attachments.length})</div>
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

function AttachmentPanel({ threadId }: { threadId: string }) {
  const [attachments, setAttachments] = useState<AttachmentMetadata[]>([]);

  useEffect(() => {
    if (!threadId) return;
    void (async () => {
      try {
        const meta = await invoke<AttachmentMetadata[]>("list_thread_attachments", {
          thread_id: threadId,
        });
        
        // Deduplicate by filename + size + type
        const unique = new Map<string, AttachmentMetadata>();
        meta.forEach((att) => {
          const key = `${att.filename || "unnamed"}-${att.size}-${att.content_type || ""}`;
          if (!unique.has(key)) {
            unique.set(key, att);
          }
        });
        
        setAttachments(Array.from(unique.values()));
      } catch (e) {
        console.error("Failed to list thread attachments:", e);
      }
    })();
  }, [threadId]);

  if (attachments.length === 0) return null;

  return (
    <div className={styles.sideAttachmentPanel}>
      <div className={styles.sidePanelHeader}>Attachments ({attachments.length})</div>
      <div className={styles.sideAttachmentList}>
        {attachments.map((att) => (
          <AttachmentItem key={att.id} att={att} />
        ))}
      </div>
    </div>
  );
}

export default function ThreadView({ thread, messages, composeOpen, composeMode, onComposeClose, onReplyClick }: Props) {
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
      <div className={styles.card} onClick={(e) => isExpanded && e.stopPropagation()}>
        <div className={styles.main}>
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
                onClick={() => onReplyClick("reply")}
              >
                Reply
              </button>
              <button
                className={styles.replyBtn}
                onClick={() => onReplyClick("replyAll")}
              >
                Reply All
              </button>
              <button
                className={styles.replyBtn}
                onClick={() => onReplyClick("forward")}
              >
                Forward
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

          {composeOpen && (
            <Compose
              thread={thread}
              messages={messages}
              mode={composeMode ?? "reply"}
              onClose={onComposeClose}
            />
          )}
        </div>

        <div className={styles.sidebarWrapper}>
          <AIPanel thread={thread} messages={messages} />
          <AttachmentPanel threadId={thread.id} />
        </div>
      </div>
    </div>
  );
}

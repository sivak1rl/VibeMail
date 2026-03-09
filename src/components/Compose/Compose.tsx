import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import DOMPurify from "dompurify";
import type { Message, Thread } from "../../stores/threads";
import { useThreadStore } from "../../stores/threads";
import { useAccountStore } from "../../stores/accounts";
import { useAiStore } from "../../stores/ai";
import { usePreferencesStore } from "../../stores/preferences";
import styles from "./Compose.module.css";

export type ComposeMode = "new" | "reply" | "replyAll" | "forward";

// ── Proofread diff types & utilities ──────────────────────────────────────────

type EqualChunk = { id: number; type: "equal"; text: string };
type ChangeChunk = { id: number; type: "change"; original: string; revised: string; accepted: boolean };
type DiffChunk = EqualChunk | ChangeChunk;

/** Split text into word + whitespace tokens, preserving all content. */
function tokenize(text: string): string[] {
  return text.split(/(\s+)/).filter((t) => t.length > 0);
}

/** LCS-based word-level diff between original and revised text. */
function computeDiff(original: string, revised: string): DiffChunk[] {
  const a = tokenize(original);
  const b = tokenize(revised);
  const m = a.length;
  const n = b.length;

  // Build LCS table
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = a[i - 1] === b[j - 1] ? dp[i - 1][j - 1] + 1 : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }

  // Backtrack to produce ops
  type Op = { type: "equal" | "delete" | "insert"; text: string };
  const ops: Op[] = [];
  let i = m, j = n;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && a[i - 1] === b[j - 1]) {
      ops.unshift({ type: "equal", text: a[i - 1] });
      i--; j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      ops.unshift({ type: "insert", text: b[j - 1] });
      j--;
    } else {
      ops.unshift({ type: "delete", text: a[i - 1] });
      i--;
    }
  }

  // Group consecutive delete+insert runs into change chunks
  const chunks: DiffChunk[] = [];
  let id = 0;
  let idx = 0;
  while (idx < ops.length) {
    if (ops[idx].type === "equal") {
      chunks.push({ id: id++, type: "equal", text: ops[idx].text });
      idx++;
    } else {
      const dels: string[] = [];
      const ins: string[] = [];
      while (idx < ops.length && ops[idx].type !== "equal") {
        if (ops[idx].type === "delete") dels.push(ops[idx].text);
        else ins.push(ops[idx].text);
        idx++;
      }
      chunks.push({
        id: id++,
        type: "change",
        original: dels.join(""),
        revised: ins.join(""),
        accepted: true,
      });
    }
  }
  return chunks;
}

function reconstructFromChunks(chunks: DiffChunk[]): string {
  return chunks.map((c) => (c.type === "equal" ? c.text : c.accepted ? c.revised : c.original)).join("");
}

// ── Markdown → Tiptap HTML converter ─────────────────────────────────────────

/** Convert plain-text with markdown-ish syntax to HTML suitable for Tiptap. */
function bodyToHtml(text: string): string {
  const lines = text.split("\n");
  const out: string[] = [];
  let inUl = false;
  let inOl = false;

  const closeList = () => {
    if (inUl) { out.push("</ul>"); inUl = false; }
    if (inOl) { out.push("</ol>"); inOl = false; }
  };

  for (const raw of lines) {
    // Blockquote
    if (raw.startsWith("> ") || raw === ">") {
      closeList();
      const inner = inlineMarkdown(raw.replace(/^>\s?/, ""));
      out.push(`<blockquote><p>${inner}</p></blockquote>`);
      continue;
    }
    // Unordered list
    const ulMatch = raw.match(/^[-*]\s+(.*)/);
    if (ulMatch) {
      if (inOl) closeList();
      if (!inUl) { out.push("<ul>"); inUl = true; }
      out.push(`<li>${inlineMarkdown(ulMatch[1])}</li>`);
      continue;
    }
    // Ordered list
    const olMatch = raw.match(/^\d+\.\s+(.*)/);
    if (olMatch) {
      if (inUl) closeList();
      if (!inOl) { out.push("<ol>"); inOl = true; }
      out.push(`<li>${inlineMarkdown(olMatch[1])}</li>`);
      continue;
    }
    closeList();
    out.push(`<p>${inlineMarkdown(raw) || "<br>"}</p>`);
  }
  closeList();
  return out.join("");
}

function inlineMarkdown(s: string): string {
  return s
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/~~(.*?)~~/g, "<s>$1</s>")
    .replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>")
    .replace(/\*(.*?)\*/g, "<em>$1</em>")
    .replace(/`(.*?)`/g, "<code>$1</code>");
}

/** Extract only the non-blockquote text from Tiptap HTML as plain text. */
function getNonQuotedText(html: string): string {
  const div = document.createElement("div");
  div.innerHTML = html;
  // Remove blockquote nodes
  div.querySelectorAll("blockquote").forEach((bq) => bq.remove());
  return div.innerText.trim();
}

/** Extract blockquote nodes as "> " prefixed plain text. */
function getQuotedText(html: string): string {
  const div = document.createElement("div");
  div.innerHTML = html;
  const quotes: string[] = [];
  div.querySelectorAll("blockquote").forEach((bq) => {
    bq.innerText.split("\n").forEach((line) => quotes.push(`> ${line}`));
  });
  return quotes.join("\n");
}

// ── Formatting toolbar ────────────────────────────────────────────────────────

function FormattingToolbar({ editor }: { editor: import("@tiptap/react").Editor | null }) {
  if (!editor) return null;
  const btn = (label: string, title: string, active: boolean, onClick: () => void) => (
    <button
      key={title}
      className={`${styles.toolbarBtn} ${active ? styles.toolbarBtnActive : ""}`}
      onMouseDown={(e) => { e.preventDefault(); onClick(); }}
      title={title}
    >
      {label}
    </button>
  );
  return (
    <div className={styles.toolbar}>
      {btn("B", "Bold", editor.isActive("bold"), () => editor.chain().focus().toggleBold().run())}
      {btn("I", "Italic", editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run())}
      {btn("S", "Strikethrough", editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run())}
      <div className={styles.toolbarDivider} />
      {btn("•", "Bullet list", editor.isActive("bulletList"), () => editor.chain().focus().toggleBulletList().run())}
      {btn("1.", "Ordered list", editor.isActive("orderedList"), () => editor.chain().focus().toggleOrderedList().run())}
      {btn("❝", "Blockquote", editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run())}
      <div className={styles.toolbarDivider} />
      {btn("< >", "Code", editor.isActive("code"), () => editor.chain().focus().toggleCode().run())}
    </div>
  );
}

// ── ProofreadReview component ─────────────────────────────────────────────────

function ProofreadReview({
  chunks,
  onToggle,
  onAcceptAll,
  onRejectAll,
  onApply,
  onCancel,
}: {
  chunks: DiffChunk[];
  onToggle: (id: number) => void;
  onAcceptAll: () => void;
  onRejectAll: () => void;
  onApply: () => void;
  onCancel: () => void;
}) {
  const changes = chunks.filter((c): c is ChangeChunk => c.type === "change");
  const accepted = changes.filter((c) => c.accepted).length;

  return (
    <div className={styles.diffReview}>
      <div className={styles.diffHeader}>
        {changes.length} change{changes.length !== 1 ? "s" : ""} · {accepted} accepted
      </div>

      <div className={styles.diffList}>
        {changes.map((chunk) => (
          <div
            key={chunk.id}
            className={`${styles.diffItem} ${chunk.accepted ? styles.diffAccepted : styles.diffRejected}`}
          >
            <div className={styles.diffContent}>
              {chunk.original ? (
                <span className={styles.diffOld}>{chunk.original}</span>
              ) : (
                <span className={styles.diffEmpty}>(empty)</span>
              )}
              <span className={styles.diffArrow}>→</span>
              {chunk.revised ? (
                <span className={styles.diffNew}>{chunk.revised}</span>
              ) : (
                <span className={styles.diffEmpty}>(removed)</span>
              )}
            </div>
            <button
              className={styles.diffToggle}
              onClick={() => onToggle(chunk.id)}
              title={chunk.accepted ? "Reject this change" : "Accept this change"}
            >
              {chunk.accepted ? "✗" : "✓"}
            </button>
          </div>
        ))}
      </div>

      <div className={styles.diffActions}>
        <button className={styles.diffBtn} onClick={onAcceptAll}>Accept All</button>
        <button className={styles.diffBtn} onClick={onRejectAll}>Reject All</button>
      </div>
      <button className={styles.aiGenerateBtn} onClick={onApply}>
        Apply {accepted} change{accepted !== 1 ? "s" : ""}
      </button>
      <button className={styles.aiProofreadBtn} onClick={onCancel}>Cancel Review</button>
    </div>
  );
}

// ── Contact autocomplete ──────────────────────────────────────────────────────

interface Props {
  thread?: Thread;
  messages?: Message[];
  mode?: ComposeMode;
  onClose: () => void;
  expanded?: boolean;
  onExpandChange?: (expanded: boolean) => void;
}

const NEW_KEY = "__new__";

function useContacts() {
  const threads = useThreadStore((s) => s.threads);
  return useMemo(() => {
    const seen = new Map<string, string | null>();
    for (const t of threads) {
      for (const p of t.participants) {
        if (!seen.has(p.email)) seen.set(p.email, p.name ?? null);
      }
    }
    return Array.from(seen.entries()).map(([email, name]) => ({ email, name }));
  }, [threads]);
}

function parseAddrs(s: string) {
  return s.split(",").map((x) => x.trim()).filter(Boolean).map((email) => ({ name: null, email }));
}

interface ContactInputProps {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
  contacts: { email: string; name: string | null }[];
  readOnly?: boolean;
}

function ContactInput({ value, onChange, placeholder, autoFocus, contacts, readOnly }: ContactInputProps) {
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const currentToken = value.split(",").pop()?.trim() ?? "";

  const suggestions = useMemo(() => {
    if (!currentToken || currentToken.length < 2) return [];
    const lower = currentToken.toLowerCase();
    return contacts
      .filter((c) => c.email.toLowerCase().includes(lower) || (c.name && c.name.toLowerCase().includes(lower)))
      .slice(0, 6);
  }, [currentToken, contacts]);

  const handleSelect = (email: string) => {
    const parts = value.split(",");
    parts[parts.length - 1] = " " + email;
    onChange(parts.join(",").trimStart() + ", ");
    setOpen(false);
    inputRef.current?.focus();
  };

  if (readOnly) return <span className={styles.metaValue}>{value}</span>;

  return (
    <div style={{ position: "relative", flex: 1 }}>
      <input
        ref={inputRef}
        className={styles.metaInput}
        value={value}
        onChange={(e) => { onChange(e.target.value); setOpen(true); }}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        onFocus={() => setOpen(true)}
        placeholder={placeholder}
        autoFocus={autoFocus}
      />
      {open && suggestions.length > 0 && (
        <div className={styles.autocomplete}>
          {suggestions.map((c) => (
            <div
              key={c.email}
              className={styles.autocompleteItem}
              onMouseDown={(e) => { e.preventDefault(); handleSelect(c.email); }}
            >
              {c.name ? `${c.name} <${c.email}>` : c.email}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Main Compose component ────────────────────────────────────────────────────

export default function Compose({
  thread,
  messages = [],
  mode: modeProp,
  onClose,
  expanded = false,
  onExpandChange,
}: Props) {
  const { activeAccountId, accounts } = useAccountStore();
  const { draftByThread, draftStreaming, draftReply, draftNew, config } = useAiStore();
  const { signatures } = usePreferencesStore();
  const contacts = useContacts();

  const mode: ComposeMode = modeProp ?? (thread ? "reply" : "new");
  const lastMsg = messages[messages.length - 1];
  const activeAccount = accounts.find((a) => a.id === activeAccountId);
  const signature = activeAccountId ? (signatures[activeAccountId] ?? "") : "";

  const [to, setTo] = useState<string>(() => {
    if (mode === "reply") return lastMsg?.from[0]?.email ?? "";
    if (mode === "replyAll") {
      return thread?.participants
        .filter((p) => p.email.toLowerCase() !== activeAccount?.email.toLowerCase())
        .map((p) => p.email)
        .join(", ") ?? "";
    }
    return "";
  });

  const [subject, setSubject] = useState<string>(() => {
    if (mode === "reply" || mode === "replyAll") {
      return thread?.subject?.startsWith("Re:") ? thread.subject : `Re: ${thread?.subject ?? ""}`;
    }
    if (mode === "forward") {
      const s = thread?.subject ?? "";
      return s.startsWith("Fwd:") ? s : `Fwd: ${s}`;
    }
    return "";
  });

  const [body, setBody] = useState<string>(() => {
    const sig = signature ? `\n\n--\n${signature}` : "";
    if (mode === "forward" && lastMsg) {
      const from = lastMsg.from.map((f) => f.name ? `${f.name} <${f.email}>` : f.email).join(", ");
      const quoted = (lastMsg.body_text ?? "").split("\n").map((l) => `> ${l}`).join("\n");
      return `${sig}\n\n---------- Forwarded message ----------\nFrom: ${from}\nDate: ${lastMsg.date ?? ""}\n\n${quoted}`;
    }
    if ((mode === "reply" || mode === "replyAll") && lastMsg) {
      const from = lastMsg.from.map((f) => f.name ? `${f.name} <${f.email}>` : f.email).join(", ");
      const quoted = (lastMsg.body_text ?? "").split("\n").map((l) => `> ${l}`).join("\n");
      return `${sig}\n\nOn ${lastMsg.date ?? ""}, ${from} wrote:\n${quoted}`;
    }
    return sig ? sig.trimStart() : "";
  });

  const [cc, setCc] = useState("");
  const [bcc, setBcc] = useState("");
  const [showCc, setShowCc] = useState(false);
  const [showBcc, setShowBcc] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);
  const [proofreading, setProofreading] = useState(false);
  const [proofreadError, setProofreadError] = useState<string | null>(null);
  const [proofreadInfo, setProofreadInfo] = useState<string | null>(null);
  const [proofreadChunks, setProofreadChunks] = useState<DiffChunk[] | null>(null);
  const [proofreadQuoted, setProofreadQuoted] = useState("");
  const [justInserted, setJustInserted] = useState(false);
  // Which draft key is streaming/displayed in the AI panel
  const [activeDraftKey, setActiveDraftKey] = useState(mode !== "new" && thread ? thread.id : NEW_KEY);

  const insertAt = useRef<number>(0);
  const prevStreaming = useRef(false);
  const suppressBodyUpdate = useRef(false);

  const draft = draftByThread[activeDraftKey];
  const isGenerating = draftStreaming[activeDraftKey] ?? false;

  const editor = useEditor({
    extensions: [StarterKit],
    content: bodyToHtml(body),
    onUpdate: ({ editor: ed }) => {
      if (suppressBodyUpdate.current) return;
      setBody(ed.getText({ blockSeparator: "\n" }));
    },
  });

  // When generation finishes, insert the completed draft at the recorded cursor position.
  useEffect(() => {
    const wasGenerating = prevStreaming.current;
    prevStreaming.current = isGenerating;
    if (wasGenerating && !isGenerating && draft && editor) {
      suppressBodyUpdate.current = true;
      const pos = insertAt.current;
      editor.chain().focus().setTextSelection(pos).insertContent(bodyToHtml(draft)).run();
      setBody(editor.getText({ blockSeparator: "\n" }));
      suppressBodyUpdate.current = false;
      setJustInserted(true);
      setTimeout(() => setJustInserted(false), 2500);
    }
  }, [draft, isGenerating, editor]);

  // Live preview body while reviewing proofread changes
  const bodyDisplay = proofreadChunks
    ? reconstructFromChunks(proofreadChunks) + (proofreadQuoted ? "\n" + proofreadQuoted : "")
    : body;

  const handleReplyAIDraft = async () => {
    if (!thread) return;
    setAiError(null);
    insertAt.current = editor?.state.selection.anchor ?? 0;
    setActiveDraftKey(thread.id);
    try {
      await draftReply(thread.id);
    } catch (e) {
      setAiError(String(e));
    }
  };

  const handleCustomGenerate = async () => {
    if (!aiPrompt.trim()) return;
    setAiError(null);
    insertAt.current = editor?.state.selection.anchor ?? 0;
    setActiveDraftKey(NEW_KEY);
    try {
      await draftNew(aiPrompt.trim());
    } catch (e) {
      setAiError(String(e));
    }
  };

  const handleProofread = async () => {
    setProofreadError(null);
    setProofreadInfo(null);
    const html = editor ? editor.getHTML() : bodyToHtml(body);
    const nonQuoted = getNonQuotedText(html);
    const quoted = getQuotedText(html);
    if (!nonQuoted.trim()) return;
    setProofreading(true);
    try {
      const result = await invoke<string>("proofread_text", { request: { text: nonQuoted } });
      const chunks = computeDiff(nonQuoted, result);
      if (!chunks.some((c) => c.type === "change")) {
        setProofreadInfo("Looks good — no changes suggested.");
        return;
      }
      setProofreadQuoted(quoted);
      setProofreadChunks(chunks);
    } catch (e) {
      setProofreadError(String(e));
    } finally {
      setProofreading(false);
    }
  };

  const toggleChunk = (id: number) => {
    setProofreadChunks((prev) =>
      prev?.map((c) => c.type === "change" && c.id === id ? { ...c, accepted: !c.accepted } : c) ?? null,
    );
  };

  const applyProofread = () => {
    if (!proofreadChunks) return;
    const applied = reconstructFromChunks(proofreadChunks);
    const full = proofreadQuoted ? applied + "\n" + proofreadQuoted : applied;
    setBody(full);
    if (editor) {
      suppressBodyUpdate.current = true;
      editor.commands.setContent(bodyToHtml(full));
      suppressBodyUpdate.current = false;
    }
    setProofreadChunks(null);
    setProofreadQuoted("");
  };

  const cancelProofread = () => {
    setProofreadChunks(null);
    setProofreadQuoted("");
  };

  const handleSend = async () => {
    if (!activeAccountId || !body.trim() || !to.trim()) return;
    setSending(true);
    setError(null);
    try {
      const isReplyMode = mode === "reply" || mode === "replyAll";
      await invoke("send_message", {
        message: {
          account_id: activeAccountId,
          to: parseAddrs(to),
          cc: cc.trim() ? parseAddrs(cc) : null,
          bcc: bcc.trim() ? parseAddrs(bcc) : null,
          subject,
          body_text: body,
          body_html: editor ? DOMPurify.sanitize(editor.getHTML()) : null,
          in_reply_to: isReplyMode ? (lastMsg?.message_id ?? null) : null,
          references: isReplyMode && lastMsg ? [lastMsg.message_id].filter(Boolean) : null,
        },
      });
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  const headerTitle = { new: "New Message", reply: "Reply", replyAll: "Reply All", forward: "Forward" }[mode];
  const isReplyMode = mode === "reply" || mode === "replyAll";

  return (
    <div className={styles.compose}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>{headerTitle}</span>
        <div className={styles.headerActions}>
          <button className={styles.expandBtn} onClick={() => onExpandChange?.(!expanded)} title={expanded ? "Collapse" : "Expand"}>
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
            <ContactInput
              value={to} onChange={setTo} placeholder="recipient@example.com"
              autoFocus={mode !== "reply" && !expanded} contacts={contacts} readOnly={mode === "reply"}
            />
            {!showCc && <button className={styles.metaToggle} onClick={() => setShowCc(true)}>Cc</button>}
            {!showBcc && <button className={styles.metaToggle} onClick={() => setShowBcc(true)}>Bcc</button>}
          </div>

          {showCc && (
            <div className={styles.meta}>
              <span className={styles.metaLabel}>Cc:</span>
              <ContactInput value={cc} onChange={setCc} placeholder="cc@example.com" contacts={contacts} />
            </div>
          )}

          {showBcc && (
            <div className={styles.meta}>
              <span className={styles.metaLabel}>Bcc:</span>
              <ContactInput value={bcc} onChange={setBcc} placeholder="bcc@example.com" contacts={contacts} />
            </div>
          )}

          <div className={styles.meta}>
            <span className={styles.metaLabel}>Subject:</span>
            {isReplyMode ? (
              <span className={styles.metaValue}>{subject}</span>
            ) : (
              <input className={styles.metaInput} value={subject} onChange={(e) => setSubject(e.target.value)} placeholder="Subject" />
            )}
          </div>

          <FormattingToolbar editor={proofreadChunks ? null : editor} />

          <div className={styles.bodyArea}>
            {proofreadChunks ? (
              <textarea
                className={styles.textarea}
                value={bodyDisplay}
                readOnly
              />
            ) : (
              <div className={styles.editorContent}>
                <EditorContent editor={editor} />
              </div>
            )}
          </div>

          {error && <div className={styles.error}>{error}</div>}

          <div className={styles.footer}>
            <div className={styles.footerRight}>
              <button className={styles.cancelBtn} onClick={onClose} disabled={sending}>Cancel</button>
              <button className={styles.sendBtn} onClick={handleSend} disabled={sending || !!proofreadChunks || !body.trim() || !to.trim()}>
                {sending ? "Sending..." : "Send"}
              </button>
            </div>
          </div>
        </div>

        {/* AI panel — shown for all modes when AI is enabled */}
        {config?.enabled && (
          <div className={styles.aiPanel}>
            <div className={styles.aiPanelHeader}>✦ AI Assistant</div>

            {proofreadChunks ? (
              <ProofreadReview
                chunks={proofreadChunks}
                onToggle={toggleChunk}
                onAcceptAll={() => setProofreadChunks((p) => p?.map((c) => c.type === "change" ? { ...c, accepted: true } : c) ?? null)}
                onRejectAll={() => setProofreadChunks((p) => p?.map((c) => c.type === "change" ? { ...c, accepted: false } : c) ?? null)}
                onApply={applyProofread}
                onCancel={cancelProofread}
              />
            ) : (
              <>
                {isReplyMode && thread && (
                  <button
                    className={styles.aiGenerateBtn}
                    onClick={handleReplyAIDraft}
                    disabled={isGenerating || proofreading}
                  >
                    {isGenerating && activeDraftKey === thread.id ? "Drafting..." : "✦ Draft Reply"}
                  </button>
                )}

                <p className={styles.aiPanelHint}>
                  {isReplyMode ? "Or describe what you want to say:" : "Describe what you want to write:"}
                </p>
                <textarea
                  className={styles.aiPrompt}
                  value={aiPrompt}
                  onChange={(e) => setAiPrompt(e.target.value)}
                  placeholder={
                    isReplyMode
                      ? "e.g. Decline politely and suggest next week"
                      : "e.g. Write to Sarah about a meeting to discuss Q4"
                  }
                  rows={4}
                />
                <button
                  className={styles.aiGenerateBtn}
                  onClick={handleCustomGenerate}
                  disabled={!aiPrompt.trim() || isGenerating || proofreading}
                >
                  {isGenerating && activeDraftKey === NEW_KEY ? "Generating..." : "✦ Generate"}
                </button>

                <button
                  className={styles.aiProofreadBtn}
                  onClick={handleProofread}
                  disabled={isGenerating || proofreading || !body.trim()}
                >
                  {proofreading ? "Proofreading..." : "✎ Proofread"}
                </button>

                {isGenerating && draft && (
                  <div className={styles.aiStream}>
                    <span className={styles.cursor}>▌</span>
                    {draft}
                  </div>
                )}

                {(aiError ?? proofreadError) && (
                  <div className={styles.error}>{aiError ?? proofreadError}</div>
                )}
                {proofreadInfo && (
                  <p className={styles.aiInsertedHint}>{proofreadInfo}</p>
                )}
                {justInserted && (
                  <p className={styles.aiInsertedHint}>↑ Inserted at cursor</p>
                )}
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

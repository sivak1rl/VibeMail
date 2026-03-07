import { useCallback, useEffect, useState } from "react";
import { useAccountStore } from "../stores/accounts";
import { useMailboxStore } from "../stores/mailboxes";
import { useThreadStore } from "../stores/threads";
import { useSearchStore } from "../stores/search";
import { useAiStore } from "../stores/ai";
import InboxList from "../components/InboxList/InboxList";
import ThreadView from "../components/ThreadView/ThreadView";
import SearchBar from "../components/SearchBar/SearchBar";
import styles from "./Inbox.module.css";

interface Props {
  onSettings: () => void;
}

export default function Inbox({ onSettings }: Props) {
  const { accounts, activeAccountId, setActiveAccount } = useAccountStore();
  const {
    mailboxes,
    selectedMailboxId,
    loading: mailboxesLoading,
    error: mailboxError,
    fetchMailboxes,
    selectMailbox,
  } = useMailboxStore();
  const {
    threads,
    selectedThreadId,
    threadMessages,
    loading,
    syncing,
    syncError,
    syncProgress,
    focusMode,
    fetchThreads,
    selectThread,
    syncAccount,
    setThreadsRead,
    setFocusMode,
    loadMoreThreads,
    hasMore,
  } = useThreadStore();
  const { results: searchResults, query: searchQuery, clear: clearSearch } = useSearchStore();
  const { loadConfig, summarizeThreads, batchSummarizing } = useAiStore();

  const [showSearch, setShowSearch] = useState(false);
  const [selectedThreadIds, setSelectedThreadIds] = useState<string[]>([]);
  const [lastSelectedThreadId, setLastSelectedThreadId] = useState<string | null>(null);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (activeAccountId) {
      setShowSearch(false);
      setSelectedThreadIds([]);
      setLastSelectedThreadId(null);
      clearSearch();
      void (async () => {
        await fetchMailboxes(activeAccountId);
        const initialMailboxId = useMailboxStore.getState().selectedMailboxId;
        await syncAccount(activeAccountId, initialMailboxId);
        await fetchMailboxes(activeAccountId, true);
      })();
    }
  }, [activeAccountId, clearSearch, fetchMailboxes, syncAccount]);

  useEffect(() => {
    if (activeAccountId && selectedMailboxId) {
      setSelectedThreadIds([]);
      setLastSelectedThreadId(null);
      void fetchThreads(activeAccountId, selectedMailboxId, focusMode);
    }
  }, [activeAccountId, selectedMailboxId, focusMode, fetchThreads]);

  const handleSync = useCallback(async () => {
    if (!activeAccountId) return;
    await syncAccount(activeAccountId, selectedMailboxId);
    await fetchMailboxes(activeAccountId, true);
  }, [activeAccountId, fetchMailboxes, selectedMailboxId, syncAccount]);

  const handleLoadMore = useCallback(() => {
    if (!activeAccountId) return;
    loadMoreThreads(activeAccountId, selectedMailboxId);
  }, [activeAccountId, selectedMailboxId, loadMoreThreads]);

  const handleMailboxSelect = useCallback((mailboxId: string) => {
    selectMailbox(mailboxId);
    setShowSearch(false);
    setSelectedThreadIds([]);
    setLastSelectedThreadId(null);
    clearSearch();
  }, [clearSearch, selectMailbox]);

  const displayedThreads = showSearch && searchQuery ? searchResults : threads;
  const selectedThread = displayedThreads.find((t) => t.id === selectedThreadId) ?? null;
  const selectedSet = new Set(selectedThreadIds);
  const selectedThreads = displayedThreads.filter((thread) => selectedSet.has(thread.id));
  const selectedUnreadThreads = selectedThreads.filter((thread) => thread.unread_count > 0);
  const allUnreadDisplayedIds = displayedThreads
    .filter((thread) => thread.unread_count > 0)
    .map((thread) => thread.id);
  const shouldMarkRead = selectedThreads.some((thread) => thread.unread_count > 0);

  const handleToggleSelect = useCallback((threadId: string, selected: boolean, withShift: boolean) => {
    const orderedIds = displayedThreads.map((thread) => thread.id);
    setSelectedThreadIds((current) => {
      if (withShift && lastSelectedThreadId) {
        const from = orderedIds.indexOf(lastSelectedThreadId);
        const to = orderedIds.indexOf(threadId);
        if (from >= 0 && to >= 0) {
          const [start, end] = from < to ? [from, to] : [to, from];
          const rangeIds = orderedIds.slice(start, end + 1);
          const next = new Set(current);
          for (const id of rangeIds) {
            if (selected) {
              next.add(id);
            } else {
              next.delete(id);
            }
          }
          return Array.from(next);
        }
      }

      const next = new Set(current);
      if (selected) {
        next.add(threadId);
      } else {
        next.delete(threadId);
      }
      return Array.from(next);
    });
    setLastSelectedThreadId(threadId);
  }, [displayedThreads, lastSelectedThreadId]);

  const handleSummarizeSelected = useCallback(async () => {
    if (selectedThreadIds.length === 0) return;
    await summarizeThreads(selectedThreadIds);
  }, [selectedThreadIds, summarizeThreads]);

  const handleSummarizeUnread = useCallback(async () => {
    if (allUnreadDisplayedIds.length === 0) return;
    await summarizeThreads(allUnreadDisplayedIds);
  }, [allUnreadDisplayedIds, summarizeThreads]);

  const handleToggleRead = useCallback(async () => {
    if (selectedThreadIds.length === 0) return;
    await setThreadsRead(selectedThreadIds, shouldMarkRead);
  }, [selectedThreadIds, setThreadsRead, shouldMarkRead]);

  return (
    <div className={styles.layout}>
      {/* Sidebar */}
      <div className={styles.sidebar}>
        <div className={styles.sidebarHeader}>
          <span className={styles.logo}>VibeMail</span>
          <button className={styles.settingsBtn} onClick={onSettings} title="Settings">
            ⚙
          </button>
        </div>

        {/* Account list */}
        {accounts.length > 1 && (
          <div className={styles.accounts}>
            {accounts.map((acc) => (
              <button
                key={acc.id}
                className={`${styles.accountBtn} ${
                  acc.id === activeAccountId ? styles.accountActive : ""
                }`}
                onClick={() => setActiveAccount(acc.id)}
              >
                {acc.email}
              </button>
            ))}
          </div>
        )}

        {/* Mailbox nav */}
        <div className={styles.nav}>
          {mailboxesLoading && <div className={styles.navHint}>Loading folders…</div>}
          {!mailboxesLoading && mailboxError && (
            <div className={styles.navHint}>Folders unavailable</div>
          )}
          {!mailboxesLoading && !mailboxError && mailboxes.length === 0 && (
            <div className={styles.navHint}>No folders yet</div>
          )}
          {mailboxes.map((mailbox) => (
            <button
              key={mailbox.id}
              className={`${styles.navItem} ${
                mailbox.id === selectedMailboxId ? styles.navActive : ""
              }`}
              onClick={() => handleMailboxSelect(mailbox.id)}
            >
              <span>{mailbox.name}</span>
              {mailbox.unread_count > 0 && (
                <span className={styles.navBadge}>{mailbox.unread_count}</span>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Thread list pane */}
      <div className={styles.listPane}>
        <div className={styles.listHeader}>
          <div className={styles.searchRow}>
            <SearchBar
              mailboxId={selectedMailboxId}
              onResults={() => setShowSearch(true)}
              onClear={() => setShowSearch(false)}
            />
          </div>
          <div className={styles.controls}>
            <button
              className={`${styles.focusBtn} ${focusMode ? styles.focusActive : ""}`}
              onClick={() => setFocusMode(!focusMode)}
              title="Focus: show only important mail"
            >
              {focusMode ? "Focus ✓" : "Focus"}
            </button>
            <button
              className={styles.syncBtn}
              onClick={handleSync}
              disabled={syncing}
              title="Sync mail"
            >
              {syncing ? "⟳" : "↻"}
            </button>
          </div>
          <div className={styles.batchRow}>
            <button
              className={styles.batchBtn}
              onClick={handleSummarizeSelected}
              disabled={selectedThreadIds.length === 0 || batchSummarizing}
              title="Summarize selected threads"
            >
              {batchSummarizing ? "Summarizing…" : `Summarize Selected (${selectedThreadIds.length})`}
            </button>
            <button
              className={styles.batchBtn}
              onClick={handleSummarizeUnread}
              disabled={allUnreadDisplayedIds.length === 0 || batchSummarizing}
              title="Summarize all unread threads"
            >
              Summarize Unread ({allUnreadDisplayedIds.length})
            </button>
            <button
              className={styles.batchBtn}
              onClick={handleToggleRead}
              disabled={selectedThreadIds.length === 0}
              title="Toggle read state for selected threads"
            >
              {selectedThreadIds.length === 0
                ? "Mark Read/Unread"
                : shouldMarkRead
                ? `Mark Read (${selectedUnreadThreads.length})`
                : "Mark Unread"}
            </button>
          </div>
          {syncing && syncProgress && (
            <div className={styles.syncStatus}>{syncProgress}</div>
          )}
          {syncError && <div className={styles.syncError}>{syncError}</div>}
        </div>

        <InboxList
          threads={displayedThreads}
          selectedId={selectedThreadId}
          selectedThreadIds={selectedThreadIds}
          onSelect={selectThread}
          onToggleSelect={handleToggleSelect}
          loading={loading}
          onLoadMore={handleLoadMore}
          hasMore={hasMore}
        />
      </div>

      {/* Thread view */}
      <div className={styles.threadPane}>
        <ThreadView thread={selectedThread} messages={threadMessages} />
      </div>
    </div>
  );
}

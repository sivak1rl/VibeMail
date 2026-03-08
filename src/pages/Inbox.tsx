import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAccountStore } from "../stores/accounts";
import { useMailboxStore } from "../stores/mailboxes";
import { useThreadStore } from "../stores/threads";
import { useSearchStore } from "../stores/search";
import { useAiStore } from "../stores/ai";
import { usePreferencesStore } from "../stores/preferences";
import { buildMailboxTree, type MailboxTreeNode } from "../utils/mailbox";
import InboxList from "../components/InboxList/InboxList";
import ThreadView from "../components/ThreadView/ThreadView";
import SearchBar from "../components/SearchBar/SearchBar";
import styles from "./Inbox.module.css";
import { invoke } from "@tauri-apps/api/core";

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
    applyThreadLabels,
    setFocusMode,
    loadMoreThreads,
    fetchHistory,
    fetchEntireMailbox,
    hasMore,
  } = useThreadStore();

  const { results: searchResults, query: searchQuery, clear: clearSearch } = useSearchStore();
  const { loadConfig, summarizeThreads, categorizeThreads, batchSummarizing, batchCategorizing } = useAiStore();
  const { autoLabelNewEmails, customCategories, historyFetchDays, historyFetchLimit } = usePreferencesStore();

  const [showSearch, setShowSearch] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [selectedThreadIds, setSelectedThreadIds] = useState<string[]>([]);
  const [lastSelectedThreadId, setLastSelectedThreadId] = useState<string | null>(null);
  const lastSyncingRef = useRef(false);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (activeAccountId) {
      setShowSearch(false);
      setSelectedThreadIds([]);
      setLastSelectedThreadId(null);
      clearSearch();
      
      const initBackgroundTasks = async () => {
        try {
          await fetchMailboxes(activeAccountId);
          
          const mailboxStore = useMailboxStore.getState();
          const initialMailboxId = mailboxStore?.selectedMailboxId || null;

          // Check background sync
          const isSyncing = await invoke<boolean>("get_sync_status", { accountId: activeAccountId });
          if (isSyncing) {
            void syncAccount(activeAccountId, initialMailboxId);
          } else {
            // We NO LONGER trigger a sync here automatically.
            // Just pull mailboxes to show the folder list.
            await fetchMailboxes(activeAccountId, true);
          }



          // Check background reindex
          const isReindexing = await invoke<boolean>("get_reindex_status", { accountId: activeAccountId });
          if (isReindexing) {
            const searchStore = useSearchStore.getState();
            if (searchStore?.reindexAll) {
              void searchStore.reindexAll(activeAccountId, false);
            }
          }
        } catch (err) {
          console.error("Inbox: Failed to initialize background tasks", err);
        }
      };

      void initBackgroundTasks();
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
    await fetchMailboxes(activeAccountId, true);
    const mailboxId = useMailboxStore.getState().selectedMailboxId;
    await syncAccount(activeAccountId, mailboxId);
    await fetchMailboxes(activeAccountId, true);
  }, [activeAccountId, fetchMailboxes, syncAccount]);

  const handleSyncAll = useCallback(async () => {
    if (!activeAccountId) return;
    await fetchMailboxes(activeAccountId, true);
    await syncAccount(activeAccountId, null); // null means all folders
    await fetchMailboxes(activeAccountId, true);
    // Refresh current view
    void fetchThreads(activeAccountId, selectedMailboxId, focusMode);
  }, [activeAccountId, fetchMailboxes, syncAccount, fetchThreads, selectedMailboxId, focusMode]);

  const handleFetchHistory = useCallback(async () => {
    if (!activeAccountId || !selectedMailboxId) return;
    await fetchHistory(activeAccountId, selectedMailboxId, historyFetchDays, historyFetchLimit);
  }, [activeAccountId, selectedMailboxId, fetchHistory, historyFetchDays, historyFetchLimit]);

  const handleFetchEntireMailbox = useCallback(async () => {
    if (!activeAccountId || !selectedMailboxId) return;
    await fetchEntireMailbox(activeAccountId, selectedMailboxId);
  }, [activeAccountId, selectedMailboxId, fetchEntireMailbox]);

  const handleLoadMore = useCallback(() => {
    if (!activeAccountId) return;
    const currentMailboxId = selectedMailboxId || null;
    if (showSearch) {
      const searchStore = useSearchStore.getState();
      if (searchStore?.loadMore) {
        void searchStore.loadMore(activeAccountId, currentMailboxId);
      }
    } else {
      loadMoreThreads(activeAccountId, currentMailboxId);
    }
  }, [activeAccountId, selectedMailboxId, loadMoreThreads, showSearch]);

  const handleMailboxSelect = useCallback((mailboxId: string) => {
    selectMailbox(mailboxId);
    setShowSearch(false);
    setSelectedThreadIds([]);
    setLastSelectedThreadId(null);
    clearSearch();
  }, [clearSearch, selectMailbox]);

  const displayedThreads = (showSearch && searchQuery ? searchResults : threads) || [];
  const selectedThread = displayedThreads.find((t) => t.id === selectedThreadId) ?? null;

  useEffect(() => {
    if (selectedThreadId) {
      console.log("Inbox: selectedThreadId changed to", selectedThreadId);
      console.log("Inbox: found in displayedThreads?", !!selectedThread);
      if (!selectedThread && displayedThreads.length > 0) {
        console.warn("Inbox: selectedThread is NULL despite having threads. First ID:", displayedThreads[0].id);
      }
    }
  }, [selectedThreadId, selectedThread, displayedThreads]);
  const selectedSet = new Set(selectedThreadIds);
  const selectedThreads = displayedThreads.filter((thread) => thread && selectedSet.has(thread.id));
  const selectedUnreadThreads = selectedThreads.filter((thread) => thread && thread.unread_count > 0);
  const allUnreadDisplayedIds = displayedThreads
    .filter((thread) => thread && thread.unread_count > 0)
    .map((thread) => thread.id);
  const shouldMarkRead = selectedThreads.some((thread) => thread && thread.unread_count > 0);
  const categoryLabels = useMemo(() => {
    const normalizeCategoryName = (value: string) =>
      value
        .toLowerCase()
        .replace(/[^a-z0-9_\- ]/g, "")
        .trim()
        .replace(/\s+/g, "_")
        .slice(0, 32);
    return new Set([
      "newsletter",
      "receipt",
      "social",
      "updates",
      ...customCategories.map((category) => normalizeCategoryName(category.name)),
    ]);
  }, [customCategories]);
  const allCategoryLabels = useMemo(() => Array.from(categoryLabels), [categoryLabels]);

  useEffect(() => {
    const wasSyncing = lastSyncingRef.current;
    lastSyncingRef.current = syncing;
    if (!autoLabelNewEmails || !wasSyncing || syncing || syncError) return;

    const candidates = threads
      .filter(
        (thread) =>
          thread.unread_count > 0 &&
          !thread.labels.some((label) => categoryLabels.has(label)),
      )
      .map((thread) => thread.id);
    if (candidates.length === 0) return;

    void (async () => {
      const results = await categorizeThreads(candidates, customCategories, false);
      const labelsByThread = Object.fromEntries(
        results.map((result) => [result.thread_id, result.label]),
      );
      applyThreadLabels(labelsByThread, allCategoryLabels);
    })();
  }, [
    autoLabelNewEmails,
    syncing,
    syncError,
    threads,
    categorizeThreads,
    applyThreadLabels,
    customCategories,
    allCategoryLabels,
    categoryLabels,
  ]);

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

  const handleCategorizeSelected = useCallback(async () => {
    if (selectedThreadIds.length === 0) return;
    const results = await categorizeThreads(selectedThreadIds, customCategories, true);
    const labelsByThread = Object.fromEntries(
      results.map((result) => [result.thread_id, result.label]),
    );
    applyThreadLabels(labelsByThread, allCategoryLabels);
  }, [
    selectedThreadIds,
    categorizeThreads,
    applyThreadLabels,
    customCategories,
    allCategoryLabels,
  ]);

  const handleCategorizeUnread = useCallback(async () => {
    if (allUnreadDisplayedIds.length === 0) return;
    const results = await categorizeThreads(allUnreadDisplayedIds, customCategories);
    const labelsByThread = Object.fromEntries(
      results.map((result) => [result.thread_id, result.label]),
    );
    applyThreadLabels(labelsByThread, allCategoryLabels);
  }, [
    allUnreadDisplayedIds,
    categorizeThreads,
    applyThreadLabels,
    customCategories,
    allCategoryLabels,
  ]);

  const handleToggleRead = useCallback(async () => {
    if (selectedThreadIds.length === 0) return;
    await setThreadsRead(selectedThreadIds, shouldMarkRead);
  }, [selectedThreadIds, setThreadsRead, shouldMarkRead]);

  const mailboxTree = useMemo(() => buildMailboxTree(mailboxes), [mailboxes]);

  const renderMailboxTree = useCallback(
    (nodes: MailboxTreeNode[], depth = 0, collapsed = false) => {
      return nodes.map((node) => (
        <div key={node.fullName}>
          <button
            className={`${styles.navItem} ${
              node.id === selectedMailboxId ? styles.navActive : ""
            } ${!node.id ? styles.navItemVirtual : ""}`}
            style={{ paddingLeft: collapsed ? "12px" : `${depth * 12 + 10}px` }}
            onClick={() => node.id && handleMailboxSelect(node.id)}
            title={collapsed ? node.name : undefined}
          >
            {collapsed ? (
              <span style={{ fontSize: "16px" }}>{node.name.charAt(0).toUpperCase()}</span>
            ) : (
              <>
                <span>{node.name}</span>
                <span
                  className={`${styles.navBadge} ${
                    !node.mailbox || node.mailbox.unread_count === 0
                      ? styles.navBadgeEmpty
                      : ""
                  }`}
                >
                  {node.mailbox?.unread_count ?? 0}
                </span>
              </>
            )}
          </button>
          {!collapsed && node.children.length > 0 && renderMailboxTree(node.children, depth + 1)}
        </div>
      ));
    },
    [selectedMailboxId, handleMailboxSelect],
  );

  return (
    <div className={styles.layout}>
      {/* Sidebar */}
      <div className={`${styles.sidebar} ${sidebarCollapsed ? styles.sidebarCollapsed : ""}`}>
        <div className={styles.sidebarHeader}>
          <button
            className={styles.toggleBtn}
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            ☰
          </button>
          {!sidebarCollapsed && <span className={styles.logo}>VibeMail</span>}
          {!sidebarCollapsed && (
            <button className={styles.settingsBtn} onClick={onSettings} title="Settings">
              ⚙
            </button>
          )}
        </div>

        {/* Account list */}
        {!sidebarCollapsed && accounts.length > 1 && (
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
          {!sidebarCollapsed && mailboxesLoading && <div className={styles.navHint}>Loading folders…</div>}
          {!sidebarCollapsed && !mailboxesLoading && mailboxError && (
            <div className={styles.navHint}>Folders unavailable</div>
          )}
          {!sidebarCollapsed && !mailboxesLoading && !mailboxError && mailboxes.length === 0 && (
            <div className={styles.navHint}>No folders yet</div>
          )}
          {renderMailboxTree(mailboxTree, 0, sidebarCollapsed)}
        </div>

        {syncing && syncProgress && (
          <div className={styles.sidebarStatus}>
            <div className={styles.statusMain}>
              <span className={styles.statusSpinner}>⟳</span>
              {!sidebarCollapsed && <span className={styles.statusText}>{syncProgress.message}</span>}
            </div>
            {!sidebarCollapsed && syncProgress.current !== null && syncProgress.total !== null && (
              <div className={styles.progressBar}>
                <div 
                  className={styles.progressFill} 
                  style={{ width: `${(syncProgress.current / syncProgress.total) * 100}%` }} 
                />
              </div>
            )}
          </div>
        )}
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
              title="Sync current mailbox"
            >
              {syncing ? "⟳" : "↻"}
            </button>
            <button
              className={styles.syncAllBtn}
              onClick={handleSyncAll}
              disabled={syncing}
              title="Sync all folders"
            >
              {syncing ? "Syncing All…" : "Sync All"}
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
              onClick={handleCategorizeSelected}
              disabled={selectedThreadIds.length === 0 || batchCategorizing}
              title="Apply category labels to selected threads"
            >
              {batchCategorizing ? "Labeling…" : `Label Selected (${selectedThreadIds.length})`}
            </button>
            <button
              className={styles.batchBtn}
              onClick={handleCategorizeUnread}
              disabled={allUnreadDisplayedIds.length === 0 || batchCategorizing}
              title="Apply category labels to all unread threads"
            >
              Label Unread ({allUnreadDisplayedIds.length})
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
          {syncError && <div className={styles.syncError}>{syncError}</div>}
        </div>

        <InboxList
          scrollKey={selectedMailboxId ?? "all"}
          threads={displayedThreads}
          selectedId={selectedThreadId}
          selectedThreadIds={selectedThreadIds}
          onSelect={selectThread}
          onToggleSelect={handleToggleSelect}
          loading={loading}
          onLoadMore={handleLoadMore}
          onRefresh={handleSync}
          onFetchHistory={handleFetchHistory}
          onFetchAll={handleFetchEntireMailbox}
          hasMore={hasMore}
          query={searchQuery}
        />
      </div>

      {/* Thread view */}
      <div className={styles.threadPane}>
        <ThreadView thread={selectedThread} messages={threadMessages} />
      </div>
    </div>
  );
}

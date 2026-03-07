import { useEffect, useState } from "react";
import { useAccountStore } from "./stores/accounts";
import { useMailboxStore } from "./stores/mailboxes";
import { useThreadStore } from "./stores/threads";
import { usePreferencesStore } from "./stores/preferences";
import Inbox from "./pages/Inbox";
import Settings from "./pages/Settings";
import AccountSetup from "./pages/AccountSetup";
import styles from "./App.module.css";

type Page = "inbox" | "settings" | "setup";

export default function App() {
  const { accounts, activeAccountId, fetchAccounts } = useAccountStore();
  const { fetchMailboxes } = useMailboxStore();
  const { syncAccount } = useThreadStore();
  const { autoSyncIntervalMinutes } = usePreferencesStore();
  const [page, setPage] = useState<Page>("inbox");
  const [initialLoad, setInitialLoad] = useState(true);

  useEffect(() => {
    fetchAccounts().then(() => setInitialLoad(false));
  }, [fetchAccounts]);

  useEffect(() => {
    if (!initialLoad && accounts.length === 0) {
      setPage("setup");
    }
  }, [accounts, initialLoad]);

  useEffect(() => {
    if (!activeAccountId || autoSyncIntervalMinutes <= 0) return;

    const intervalMs = autoSyncIntervalMinutes * 60 * 1000;
    const timer = setInterval(() => {
      if (useThreadStore.getState().syncing) return;
      void (async () => {
        const mailboxId = useMailboxStore.getState().selectedMailboxId;
        await syncAccount(activeAccountId, mailboxId);
        await fetchMailboxes(activeAccountId, true);
      })();
    }, intervalMs);

    return () => clearInterval(timer);
  }, [activeAccountId, autoSyncIntervalMinutes, fetchMailboxes, syncAccount]);

  if (initialLoad) {
    return <div className={styles.app} />;
  }

  return (
    <div className={styles.app}>
      {page === "setup" && <AccountSetup onDone={() => setPage("inbox")} />}
      {page === "inbox" && (
        <Inbox onSettings={() => setPage("settings")} />
      )}
      {page === "settings" && (
        <Settings onBack={() => setPage("inbox")} />
      )}
    </div>
  );
}

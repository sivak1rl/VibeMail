import { useEffect, useState, useRef, Component, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAccountStore } from "./stores/accounts";
import { useMailboxStore } from "./stores/mailboxes";
import { useThreadStore } from "./stores/threads";
import { usePreferencesStore } from "./stores/preferences";
import Inbox from "./pages/Inbox";
import Settings from "./pages/Settings";
import AccountSetup from "./pages/AccountSetup";
import styles from "./App.module.css";

class ErrorBoundary extends Component<{ children: ReactNode }, { hasError: boolean; error: Error | null }> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }
  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }
  render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: "40px", color: "#ff6b6b", background: "#1a1a1a", height: "100vh", width: "100vw", overflow: "auto" }}>
          <h2>Application Error</h2>
          <pre style={{ fontSize: "12px", marginTop: "20px", whiteSpace: "pre-wrap" }}>
            {this.state.error?.stack}
          </pre>
          <button 
            onClick={() => window.location.reload()}
            style={{ marginTop: "20px", padding: "8px 16px", cursor: "pointer" }}
          >
            Reload Application
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

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

  // IMAP IDLE — start push notifications for the active account.
  const idleAccountRef = useRef<string | null>(null);
  useEffect(() => {
    if (!activeAccountId) return;

    // Start IDLE for this account.
    invoke("start_idle", { accountId: activeAccountId }).catch(() => {});
    idleAccountRef.current = activeAccountId;

    // Listen for new-mail push events from IDLE.
    const unlisten = listen<{ account_id: string }>("idle-new-mail", (event) => {
      const { account_id } = event.payload;
      if (useThreadStore.getState().syncing) return;
      void (async () => {
        const mailboxId = useMailboxStore.getState().selectedMailboxId;
        await syncAccount(account_id, mailboxId);
        await fetchMailboxes(account_id, true);
      })();
    });

    return () => {
      // Stop IDLE when account changes or component unmounts.
      if (idleAccountRef.current) {
        invoke("stop_idle", { accountId: idleAccountRef.current }).catch(() => {});
        idleAccountRef.current = null;
      }
      unlisten.then((fn) => fn());
    };
  }, [activeAccountId, syncAccount, fetchMailboxes]);

  if (initialLoad) {
    return <div className={styles.app} />;
  }

  return (
    <ErrorBoundary>
      <div className={styles.app}>
        {page === "setup" && <AccountSetup onDone={() => setPage("inbox")} />}
        {page === "inbox" && (
          <Inbox onSettings={() => setPage("settings")} />
        )}
        {page === "settings" && (
          <Settings 
            onBack={() => setPage("inbox")} 
            onReset={async () => {
              await fetchAccounts();
              setPage("inbox");
            }}
          />
        )}

      </div>
    </ErrorBoundary>
  );
}

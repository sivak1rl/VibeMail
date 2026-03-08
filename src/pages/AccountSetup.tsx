import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";
import { useAccountStore } from "../stores/accounts";
import styles from "./AccountSetup.module.css";
import logoTransparent from "../../logo_transparent.png";

interface Props {
  onDone: () => void;
}

type Step = "choose" | "oauth_waiting" | "generic";

export default function AccountSetup({ onDone }: Props) {
  const { fetchAccounts } = useAccountStore();
  const [step, setStep] = useState<Step>("choose");
  const [provider, setProvider] = useState<"gmail" | "outlook" | "generic">("gmail");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [genericForm, setGenericForm] = useState({
    name: "",
    email: "",
    imap_host: "",
    imap_port: "993",
    smtp_host: "",
    smtp_port: "587",
    password: "",
  });
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const handleOAuthStart = async (p: "gmail" | "outlook") => {
    setError(null);
    setLoading(true);
    setStatus("Opening browser…");
    try {
      const res = await invoke<{ url: string; account_id: string }>("get_oauth_url", {
        provider: p,
        client_id: clientId,
        client_secret: clientSecret || null,
        });

        // Start listener BEFORE opening browser so port 7887 is ready

      const redirectPromise = invoke("await_oauth_redirect", {
        accountId: res.account_id,
      });

      // Open browser
      await open(res.url);

      setStatus("Waiting for you to approve access in the browser…");
      setStep("oauth_waiting");

      await redirectPromise;

      await fetchAccounts();
      onDone();
    } catch (e) {
      setError(String(e));
      setStatus(null);
      setStep("choose");
    } finally {
      setLoading(false);
    }
  };

  const handleGenericAdd = async () => {
    setError(null);
    setLoading(true);
    try {
      await invoke("add_account", {
        request: {
          name: genericForm.name,
          email: genericForm.email,
          provider: "generic",
          imap_host: genericForm.imap_host,
          imap_port: parseInt(genericForm.imap_port),
          smtp_host: genericForm.smtp_host,
          smtp_port: parseInt(genericForm.smtp_port),
          password: genericForm.password,
          client_id: null,
          client_secret: null,
        },
      });
      await fetchAccounts();
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.logoRow}>
          <img className={styles.logo} src={logoTransparent} alt="VibeMail" />
        </div>
        <h1 className={styles.title}>Add your first account</h1>

        {step === "choose" && (
          <>
            <div className={styles.providerBtns}>
              <button
                className={`${styles.providerBtn} ${provider === "gmail" ? styles.providerActive : ""}`}
                onClick={() => setProvider("gmail")}
              >
                Gmail
              </button>
              <button
                className={`${styles.providerBtn} ${provider === "outlook" ? styles.providerActive : ""}`}
                onClick={() => setProvider("outlook")}
              >
                Outlook / M365
              </button>
              <button
                className={`${styles.providerBtn} ${provider === "generic" ? styles.providerActive : ""}`}
                onClick={() => { setProvider("generic"); setStep("generic"); }}
              >
                IMAP / Other
              </button>
            </div>

            {(provider === "gmail" || provider === "outlook") && (
              <>
                <label className={styles.label}>
                  OAuth Client ID
                  <input
                    className={styles.input}
                    type="text"
                    placeholder={
                      provider === "gmail"
                        ? "####.apps.googleusercontent.com"
                        : "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                    }
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                  />
                </label>
                <label className={styles.label}>
                  Client Secret
                  <input
                    className={styles.input}
                    type="password"
                    placeholder={provider === "gmail" ? "GOCSPX-…" : "optional for Outlook"}
                    value={clientSecret}
                    onChange={(e) => setClientSecret(e.target.value)}
                  />
                  <span className={styles.hint}>
                    {provider === "gmail"
                      ? "Required for Google desktop OAuth. Create one in Google Cloud Console → APIs & Services → Credentials."
                      : "Optional. Register in Azure AD (Entra ID) → App registrations."}
                  </span>
                </label>
                <button
                  className={styles.primaryBtn}
                  disabled={!clientId || loading}
                  onClick={() => handleOAuthStart(provider as "gmail" | "outlook")}
                >
                  {loading
                    ? status ?? "Connecting…"
                    : `Connect with ${provider === "gmail" ? "Google" : "Microsoft"}`}
                </button>
              </>
            )}
          </>
        )}

        {step === "oauth_waiting" && (
          <div className={styles.waiting}>
            <div className={styles.spinner} />
            <p className={styles.waitingText}>
              {status ?? "Waiting for browser authorization…"}
            </p>
            <p className={styles.waitingHint}>
              A browser window opened. Approve access, then return here — the app
              will complete setup automatically.
            </p>
            <button
              className={styles.backBtn}
              onClick={() => { setStep("choose"); setLoading(false); setStatus(null); }}
            >
              Cancel
            </button>
          </div>
        )}

        {step === "generic" && (
          <>
            {(["name", "email", "imap_host", "imap_port", "smtp_host", "smtp_port"] as const).map(
              (key) => (
                <label key={key} className={styles.label}>
                  {key.replace(/_/g, " ")}
                  <input
                    className={styles.input}
                    type={key.includes("port") ? "number" : "text"}
                    value={genericForm[key]}
                    onChange={(e) =>
                      setGenericForm((f) => ({ ...f, [key]: e.target.value }))
                    }
                  />
                </label>
              )
            )}
            <label className={styles.label}>
              Password / App password
              <input
                className={styles.input}
                type="password"
                value={genericForm.password}
                onChange={(e) =>
                  setGenericForm((f) => ({ ...f, password: e.target.value }))
                }
              />
            </label>
            <button
              className={styles.primaryBtn}
              onClick={handleGenericAdd}
              disabled={loading || !genericForm.email}
            >
              {loading ? "Adding…" : "Add Account"}
            </button>
            <button className={styles.backBtn} onClick={() => setStep("choose")}>
              Back
            </button>
          </>
        )}

        {error && <div className={styles.error}>{error}</div>}
      </div>
    </div>
  );
}

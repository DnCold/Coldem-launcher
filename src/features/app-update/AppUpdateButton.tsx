import { useEffect, useState } from "react";
import { CircleAlert, CloudDownload, LoaderCircle, ShieldCheck, Sparkles, X } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { PetSticker } from "../../components/PetSticker";
import { appUpdateClient, type LauncherUpdateMetadata } from "../../lib/appUpdateClient";

type UpdatePhase =
  | "loading"
  | "disabled"
  | "checking"
  | "current"
  | "available"
  | "installing"
  | "error";

interface LauncherUpdateEvent {
  state: "downloading" | "installed";
  downloaded?: number;
  total?: number | null;
}

export function AppUpdateButton() {
  const [phase, setPhase] = useState<UpdatePhase>("loading");
  const [update, setUpdate] = useState<LauncherUpdateMetadata | null>(null);
  const [message, setMessage] = useState("Reading launcher version");
  const [showPanel, setShowPanel] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);

  const check = async () => {
    setPhase("checking");
    setMessage("Checking for a Coldem update");
    try {
      const found = await appUpdateClient.check();
      setUpdate(found);
      setPhase(found ? "available" : "current");
      setMessage(found ? `Coldem ${found.version} is ready` : "Coldem is up to date");
      setShowPanel(Boolean(found));
    } catch (reason) {
      setPhase("error");
      setMessage(String(reason));
    }
  };

  useEffect(() => {
    let active = true;
    void appUpdateClient.status().then((status) => {
      if (!active) return;
      if (!status.configured) {
        setPhase("disabled");
        setMessage(`Coldem ${status.currentVersion} · signed updates activate in release builds`);
        return;
      }
      void check();
    }).catch((reason) => {
      if (!active) return;
      setPhase("error");
      setMessage(String(reason));
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) return;
    let unlisten: (() => void) | undefined;
    void listen<LauncherUpdateEvent>("launcher://self-update", (event) => {
      if (event.payload.state === "downloading") {
        const ratio = event.payload.total
          ? Math.min(1, (event.payload.downloaded ?? 0) / event.payload.total)
          : null;
        setProgress(ratio);
        setMessage(ratio == null ? "Downloading the Coldem update" : `Downloading Coldem · ${Math.round(ratio * 100)}%`);
      } else {
        setProgress(1);
        setMessage("Update installed · restarting Coldem");
      }
    }).then((stop) => { unlisten = stop; });
    return () => unlisten?.();
  }, []);

  const activate = async () => {
    if (phase === "available" && update) {
      setPhase("installing");
      setShowPanel(true);
      setProgress(0);
      setMessage(`Installing Coldem ${update.version}`);
      try {
        await appUpdateClient.install();
      } catch (reason) {
        setPhase("error");
        setMessage(String(reason));
      }
      return;
    }
    await check();
  };

  const busy = phase === "loading" || phase === "checking" || phase === "installing";
  const Icon = busy
    ? LoaderCircle
    : phase === "available"
      ? CloudDownload
      : phase === "error"
        ? CircleAlert
        : ShieldCheck;

  return (
    <>
      <button
        type="button"
        className={`icon-button app-update-button app-update-button--${phase}`}
        onClick={() => phase === "available" ? setShowPanel(true) : void activate()}
        disabled={busy || phase === "disabled"}
        aria-label={message}
        title={message}
      >
        <Icon size={18} className={busy ? "spin" : ""} />
        {phase === "available" && <i aria-hidden="true" />}
      </button>

      {showPanel && update && (
        <div className="dialog-backdrop update-dialog-backdrop" role="presentation" onMouseDown={() => setShowPanel(false)}>
          <section className="dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="dialog-graffiti" aria-hidden="true"><span>NEW DROP!</span><i>///</i><b>✦</b></div>
            <header className="dialog__header">
              <div>
                <p className="eyebrow"><Sparkles size={12} /> LAUNCHER UPDATE</p>
                <h2 id="update-title">A colder build landed.</h2>
              </div>
              <button type="button" className="icon-button" onClick={() => setShowPanel(false)} aria-label="Close"><X size={19} /></button>
            </header>
            <div className="update-dialog__body">
              <PetSticker kind="yang" variant={4} decorative />
              <div>
                <span className="update-version">v{update.currentVersion} <b>→</b> v{update.version}</span>
                <p>Download the signed Coldem release and restart with the newest launcher features.</p>
                <small>Verified through GitHub Releases.</small>
                {phase === "installing" && (
                  <div className="update-progress" role="progressbar" aria-label="Launcher update progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress == null ? undefined : Math.round(progress * 100)}>
                    <i style={{ width: `${Math.round((progress ?? .12) * 100)}%` }} />
                    <span>{progress == null ? "DOWNLOADING" : `${Math.round(progress * 100)}%`}</span>
                  </div>
                )}
              </div>
            </div>
            <footer className="dialog__footer">
              <button type="button" className="secondary-button" onClick={() => setShowPanel(false)}>Later</button>
              <button type="button" className="primary-button" onClick={() => void activate()} disabled={busy}>
                {phase === "installing" ? <><LoaderCircle size={17} className="spin" /> Installing...</> : <><CloudDownload size={17} /> Update Coldem</>}
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}

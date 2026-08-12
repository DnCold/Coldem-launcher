import { useEffect, useState } from "react";
import { CircleAlert, CloudDownload, LoaderCircle, ShieldCheck } from "lucide-react";
import { appUpdateClient, type LauncherUpdateMetadata } from "../../lib/appUpdateClient";

type UpdatePhase =
  | "loading"
  | "disabled"
  | "checking"
  | "current"
  | "available"
  | "installing"
  | "error";

export function AppUpdateButton() {
  const [phase, setPhase] = useState<UpdatePhase>("loading");
  const [update, setUpdate] = useState<LauncherUpdateMetadata | null>(null);
  const [message, setMessage] = useState("Reading launcher version");

  const check = async () => {
    setPhase("checking");
    setMessage("Checking for a Coldem update");
    try {
      const found = await appUpdateClient.check();
      setUpdate(found);
      setPhase(found ? "available" : "current");
      setMessage(found ? `Coldem ${found.version} is ready` : "Coldem is up to date");
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

  const activate = async () => {
    if (phase === "available" && update) {
      setPhase("installing");
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
    <button
      type="button"
      className={`icon-button app-update-button app-update-button--${phase}`}
      onClick={() => void activate()}
      disabled={busy || phase === "disabled"}
      aria-label={message}
      title={message}
    >
      <Icon size={18} className={busy ? "spin" : ""} />
      {phase === "available" && <i aria-hidden="true" />}
    </button>
  );
}

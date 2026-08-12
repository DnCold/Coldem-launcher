import { useEffect, useMemo, useState } from "react";
import { Check, Download, HardDrive, X } from "lucide-react";
import type { InstallOptions, InstallPlan, Upload } from "../../types/launcher";

interface InstallDialogProps {
  options: InstallOptions;
  onPlan: (uploadId: number) => Promise<InstallPlan>;
  onInstall: (upload: Upload) => void;
  onClose: () => void;
}

const formatBytes = (bytes?: number) => {
  if (!bytes) return "Calculating…";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index > 2 ? 1 : 0)} ${units[index]}`;
};

const platformLabel = (upload: Upload) => {
  const names = Object.entries(upload.platforms ?? {})
    .filter(([, enabled]) => enabled)
    .map(([name]) => (name === "osx" ? "macOS" : name[0].toUpperCase() + name.slice(1)));
  return names.join(" · ") || "This device";
};

export function InstallDialog({
  options,
  onPlan,
  onInstall,
  onClose
}: InstallDialogProps) {
  const [selectedId, setSelectedId] = useState(options.uploads[0]?.id);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [planning, setPlanning] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const selected = useMemo(
    () => options.uploads.find((upload) => upload.id === selectedId),
    [options.uploads, selectedId]
  );

  useEffect(() => {
    if (!selectedId) return;
    let active = true;
    setPlanning(true);
    setPlan(null);
    setError(null);
    void onPlan(selectedId)
      .then((nextPlan) => {
        if (active) setPlan(nextPlan);
      })
      .catch((reason) => {
        if (active) setError(String(reason));
      })
      .finally(() => {
        if (active) setPlanning(false);
      });
    return () => {
      active = false;
    };
  }, [onPlan, selectedId]);

  const confirm = () => {
    if (!selected) return;
    onInstall(selected);
    onClose();
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="dialog install-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="install-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="dialog__header">
          <div>
            <p className="eyebrow">READY TO INSTALL</p>
            <h2 id="install-title">{options.game.title}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close">
            <X size={19} />
          </button>
        </header>

        <div className="install-dialog__body">
          <p className="field-label">Choose a version</p>
          <div className="upload-list">
            {options.uploads.map((upload) => (
              <button
                type="button"
                key={upload.id}
                className={`upload-option ${selectedId === upload.id ? "upload-option--selected" : ""}`}
                onClick={() => setSelectedId(upload.id)}
              >
                <span className="upload-option__check">
                  {selectedId === upload.id && <Check size={13} strokeWidth={3} />}
                </span>
                <span>
                  <strong>{upload.displayName || upload.filename}</strong>
                  <small>{platformLabel(upload)} · {formatBytes(upload.size)}</small>
                </span>
              </button>
            ))}
          </div>

          <div className="install-summary">
            <div>
              <HardDrive size={17} />
              <span>
                <small>Space after install</small>
                <strong>
                  {planning ? "Calculating…" : formatBytes(plan?.info.diskUsage?.finalDiskUsage)}
                </strong>
              </span>
            </div>
            <div>
              <Download size={17} />
              <span>
                <small>Download</small>
                <strong>{formatBytes(selected?.size)}</strong>
              </span>
            </div>
          </div>
          {error && <p className="form-error">{error}</p>}
        </div>

        <footer className="dialog__footer">
          <button type="button" className="secondary-button" onClick={onClose}>Not now</button>
          <button
            type="button"
            className="primary-button"
            onClick={confirm}
            disabled={!selected || planning || !!plan?.info.error}
          >
            Install game
          </button>
        </footer>
      </section>
    </div>
  );
}

import { Eye, Palette, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";

interface SettingsDialogProps {
  onClose: () => void;
}

const readSetting = (key: string, fallback: boolean) => {
  try {
    const saved = window.localStorage.getItem(key);
    return saved == null ? fallback : saved === "true";
  } catch {
    return fallback;
  }
};

const applySetting = (key: string, value: boolean) => {
  document.documentElement.setAttribute(`data-${key}`, value ? "on" : "off");
  try {
    window.localStorage.setItem(key, String(value));
  } catch {
    // Preferences are optional; the current launcher session can still use them.
  }
};

function ToggleRow({
  icon,
  title,
  detail,
  checked,
  onChange
}: {
  icon: ReactNode;
  title: string;
  detail: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <div className="settings-dialog__row">
      <span className="settings-dialog__row-icon">{icon}</span>
      <div><strong>{title}</strong><small>{detail}</small></div>
      <button type="button" className={checked ? "settings-toggle active" : "settings-toggle"} onClick={onChange} aria-pressed={checked}>
        <i /> <span>{checked ? "ON" : "OFF"}</span>
      </button>
    </div>
  );
}

export function SettingsDialog({ onClose }: SettingsDialogProps) {
  const [clarity, setClarity] = useState(() => readSetting("coldem-clarity", false));
  const [effects, setEffects] = useState(() => readSetting("coldem-effects", true));

  useEffect(() => applySetting("coldem-clarity", clarity), [clarity]);
  useEffect(() => applySetting("coldem-effects", effects), [effects]);

  return (
    <div className="dialog-backdrop settings-backdrop" role="presentation">
      <section className="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header>
          <div><p>CONTROL DECK // LOCAL PREFS</p><h2 id="settings-title">Tune Coldem</h2></div>
          <button type="button" onClick={onClose} aria-label="Close settings"><X size={18} /></button>
        </header>
        <p className="settings-dialog__intro">These controls stay on this computer and only change how the launcher looks.</p>
        <div className="settings-dialog__list">
          <ToggleRow icon={<Eye size={17} />} title="High-clarity type" detail="Boost small labels and supporting text." checked={clarity} onChange={() => setClarity((value) => !value)} />
          <ToggleRow icon={<Palette size={17} />} title="Street effects" detail="Keep the graffiti, glow, grain, and motion alive." checked={effects} onChange={() => setEffects((value) => !value)} />
        </div>
        <footer><span><i /> SETTINGS SAVED LOCALLY</span><b>DNCLD // 01</b></footer>
      </section>
    </div>
  );
}

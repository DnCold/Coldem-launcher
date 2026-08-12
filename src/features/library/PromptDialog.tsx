import { FileText, ShieldCheck, X } from "lucide-react";
import type { ButlerPrompt, Upload } from "../../types/launcher";
import { PetSticker } from "../../components/PetSticker";

interface PromptDialogProps {
  prompt: ButlerPrompt;
  onRespond: (result: Record<string, unknown>) => Promise<void>;
}

interface ActionChoice {
  name?: string;
  path?: string;
}

export function PromptDialog({ prompt, onRespond }: PromptDialogProps) {
  const isLicense = prompt.method === "AcceptLicense";
  const isSandbox = prompt.method === "AllowSandboxSetup";
  const isPrereqsFailure = prompt.method === "PrereqsFailed";
  const actions = (prompt.params.actions ?? []) as ActionChoice[];
  const uploads = (prompt.params.uploads ?? []) as Upload[];
  const choices = actions.length ? actions : uploads;

  const decline = () => {
    if (isLicense) return onRespond({ accept: false });
    if (isSandbox) return onRespond({ allow: false });
    if (isPrereqsFailure) return onRespond({ continue: false });
    return onRespond({ index: -1 });
  };

  const accept = () => {
    if (isLicense) return onRespond({ accept: true });
    if (isPrereqsFailure) return onRespond({ continue: true });
    return onRespond({ allow: true });
  };

  return (
    <div className="dialog-backdrop">
      <section className="dialog prompt-dialog" role="dialog" aria-modal="true">
        <div className="dialog-graffiti" aria-hidden="true"><span>YOUR CALL</span><i>✦ ✦</i><b>?</b></div>
        <PetSticker kind="yin" variant={3} className="dialog-pet dialog-pet--prompt" decorative />
        <header className="dialog__header">
          <div className="prompt-dialog__title">
            {isLicense ? <FileText size={21} /> : <ShieldCheck size={21} />}
            <div>
              <p className="eyebrow">GAME NEEDS YOUR INPUT</p>
              <h2>{isLicense ? "License agreement" : isSandbox ? "Allow sandbox setup?" : isPrereqsFailure ? "A prerequisite failed" : "Choose what to launch"}</h2>
            </div>
          </div>
          <button type="button" className="icon-button" onClick={() => void decline()} aria-label="Cancel">
            <X size={19} />
          </button>
        </header>

        <div className="prompt-dialog__body">
          {isLicense && <pre>{String(prompt.params.text ?? "")}</pre>}
          {isSandbox && (
            <p>This game requested an isolated runtime. Your operating system may ask for permission next.</p>
          )}
          {isPrereqsFailure && (
            <p>{String(prompt.params.error ?? "A required component could not be installed.")} You can stop here or try launching the game anyway.</p>
          )}
          {choices.length > 0 && (
            <div className="prompt-choices">
              {choices.map((choice, index) => {
                const label = "displayName" in choice
                  ? choice.displayName || choice.filename
                  : choice.name || choice.path || `Option ${index + 1}`;
                return (
                  <button key={index} type="button" onClick={() => void onRespond({ index })}>
                    <span>{label}</span>
                    <small>Choose</small>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {(isLicense || isSandbox || isPrereqsFailure) && (
          <footer className="dialog__footer">
            <button type="button" className="secondary-button" onClick={() => void decline()}>Decline</button>
            <button type="button" className="primary-button" onClick={() => void accept()}>
              {isLicense ? "Accept and continue" : isSandbox ? "Allow setup" : "Launch anyway"}
            </button>
          </footer>
        )}
      </section>
    </div>
  );
}

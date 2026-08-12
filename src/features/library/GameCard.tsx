import {
  AlertCircle,
  ArrowDownToLine,
  CirclePlay,
  Download,
  ExternalLink,
  RotateCw
} from "lucide-react";
import { PetSticker, type PetStickerKind } from "../../components/PetSticker";
import { ProgressRing } from "../../components/ProgressRing";
import type {
  ActiveOperation,
  Cave,
  GameRecord,
  GameUpdate
} from "../../types/launcher";
import { deriveGameStatus } from "./gameState";

interface GameCardProps {
  game: GameRecord;
  cave?: Cave;
  update?: GameUpdate;
  operation?: ActiveOperation;
  onInstall: () => void;
  onUpdate: () => void;
  onPlay: () => void;
  onOpenPage?: () => void;
}

const actionCopy = {
  not_installed: "Install",
  installed: "Play",
  update_available: "Update",
  installing: "Installing",
  updating: "Updating",
  launching: "Starting",
  playing: "Running",
  failed: "Retry"
} as const;

const stickerKinds: PetStickerKind[] = ["yin", "yang", "deadpool", "wolverine"];

export function GameCard({
  game,
  cave,
  update,
  operation,
  onInstall,
  onUpdate,
  onPlay,
  onOpenPage
}: GameCardProps) {
  const status = deriveGameStatus(game.id, cave, update, operation);
  const isBusy = ["installing", "updating", "launching", "playing"].includes(status);
  const progress = operation?.progress ?? (status === "playing" ? 1 : 0);
  const creator = cave?.game.user?.displayName || cave?.game.user?.username || "DanCold";

  const runAction = () => {
    if (status === "not_installed") return onInstall();
    if (status === "update_available") return onUpdate();
    if (status === "failed") return cave ? onUpdate() : onInstall();
    if (status === "installed") return onPlay();
  };

  const icon = () => {
    if (isBusy) return <ProgressRing value={progress} />;
    if (status === "installed") return <CirclePlay size={17} fill="currentColor" />;
    if (status === "update_available") return <ArrowDownToLine size={17} />;
    if (status === "failed") return <AlertCircle size={17} />;
    return <Download size={17} />;
  };

  return (
    <article className={`game-card game-card--${status}`}>
      <div className={`game-card__cover cover-fallback cover-fallback--${game.id % 8}`}>
        {game.cover ? (
          <img src={game.cover} alt="" loading="lazy" draggable={false} />
        ) : (
          <div className="cover-monogram" aria-hidden="true">
            <span>{game.title.slice(0, 1)}</span>
            <i />
          </div>
        )}
        <div className="game-card__shade" />
        <span className="game-card__genre">
          {update ? "Update ready" : cave ? "In rotation" : "New world"}
        </span>
        <PetSticker
          kind={stickerKinds[game.id % stickerKinds.length]}
          seed={game.id}
          className="game-card__sticker"
          decorative
        />
        {onOpenPage && (
          <button
            type="button"
            className="game-page-link"
            onClick={onOpenPage}
            aria-label={`Open ${game.title} page`}
            title="Open game page"
          >
            <ExternalLink size={14} />
          </button>
        )}
        {operation?.state === "failed" && (
          <span className="failed-pill">Needs attention</span>
        )}
      </div>

      <div className="game-card__meta">
        <div className="game-card__title-row">
          <div>
            <h3 title={game.title}>{game.title}</h3>
            <p>{creator}</p>
          </div>
          {cave && !update && <span className="installed-dot" title="Installed" />}
        </div>

        <div className="game-card__archive" aria-label={`${game.title} archive details`}>
          <span>#{String(game.id).slice(-4).padStart(4, "0")}</span>
          <i>{update ? "PATCH" : cave ? "ARCHIVED" : "UNLOCK"}</i>
        </div>

        <button
          type="button"
          className={`game-action game-action--${status}`}
          disabled={isBusy}
          onClick={runAction}
        >
          {icon()}
          <span>{actionCopy[status]}</span>
          {isBusy && operation?.progress != null && operation.kind !== "play" && (
            <small>{Math.round(operation.progress * 100)}%</small>
          )}
          {status === "failed" && <RotateCw size={14} className="game-action__end" />}
        </button>
      </div>
    </article>
  );
}

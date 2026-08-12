import type {
  ActiveOperation,
  Cave,
  GameUpdate
} from "../../types/launcher";

export type GameStatus =
  | "not_installed"
  | "installed"
  | "update_available"
  | "installing"
  | "updating"
  | "launching"
  | "playing"
  | "failed";

export function deriveGameStatus(
  _gameId: number,
  cave: Cave | undefined,
  update: GameUpdate | undefined,
  operation: ActiveOperation | undefined
): GameStatus {
  if (operation && operation.state !== "finished") {
    if (operation.state === "failed") return "failed";
    if (operation.kind === "install") return "installing";
    if (operation.kind === "update") return "updating";
    return operation.state === "running" ? "playing" : "launching";
  }
  if (update) return "update_available";
  if (cave) return "installed";
  return "not_installed";
}

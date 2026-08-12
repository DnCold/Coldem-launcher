import { describe, expect, it } from "vitest";
import { deriveGameStatus } from "./gameState";

describe("deriveGameStatus", () => {
  it("prioritizes live operations over persisted state", () => {
    expect(
      deriveGameStatus(1, {} as never, {} as never, {
        gameId: 1,
        kind: "update",
        state: "working",
        progress: 0.4,
        startedAt: 0
      })
    ).toBe("updating");
  });

  it("shows updates before the installed state", () => {
    expect(deriveGameStatus(1, {} as never, {} as never, undefined)).toBe(
      "update_available"
    );
  });

  it("falls back to install for games without caves", () => {
    expect(deriveGameStatus(1, undefined, undefined, undefined)).toBe(
      "not_installed"
    );
  });
});

export type PetStickerKind = "yin" | "yang" | "deadpool" | "wolverine";

interface PetStickerProps {
  kind: PetStickerKind;
  className?: string;
  decorative?: boolean;
}

const petLabels: Record<PetStickerKind, string> = {
  yin: "Yin, the white cat",
  yang: "Yang, the black cat",
  deadpool: "Deadpool, the orange hamster",
  wolverine: "Wolverine, the fluffy gray hamster"
};

export function PetSticker({
  kind,
  className = "",
  decorative = false
}: PetStickerProps) {
  return (
    <span
      className={`pet-sticker pet-sticker--${kind} ${className}`.trim()}
      aria-hidden={decorative || undefined}
      role={decorative ? undefined : "img"}
      aria-label={decorative ? undefined : petLabels[kind]}
      title={decorative ? undefined : petLabels[kind]}
    />
  );
}

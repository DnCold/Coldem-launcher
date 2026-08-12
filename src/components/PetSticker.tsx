export type PetStickerKind = "yin" | "yang" | "deadpool" | "wolverine";
export type PetStickerVariant = 1 | 2 | 3 | 4;

interface PetStickerProps {
  kind: PetStickerKind;
  variant?: PetStickerVariant;
  seed?: string | number;
  className?: string;
  decorative?: boolean;
}

const petLabels: Record<PetStickerKind, string> = {
  yin: "Yin, the chill black cat with amber-green eyes",
  yang: "Yang, the serious white cat with green eyes",
  deadpool: "Deadpool, the orange hamster",
  wolverine: "Wolverine, the fluffy gray hamster"
};

const hashSeed = (value: string) => {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
};

export function PetSticker({
  kind,
  variant,
  seed,
  className = "",
  decorative = false
}: PetStickerProps) {
  const selectedVariant = variant ?? ((hashSeed(`${kind}:${seed ?? kind}`) % 4) + 1) as PetStickerVariant;

  return (
    <span
      className={`pet-sticker pet-sticker--${kind} pet-sticker--variant-${selectedVariant} ${kind === "yin" || kind === "yang" ? `pet-sticker--${kind}-variant-${selectedVariant}` : ""} ${className}`.trim()}
      aria-hidden={decorative || undefined}
      role={decorative ? undefined : "img"}
      aria-label={decorative ? undefined : petLabels[kind]}
      title={decorative ? undefined : petLabels[kind]}
    />
  );
}

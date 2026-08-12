import dancoldLogo from "../assets/dancold-logo.png";

interface BrandMarkProps {
  compact?: boolean;
}

export function BrandMark({ compact = false }: BrandMarkProps) {
  return (
    <div className={`brand ${compact ? "brand--compact" : ""}`} aria-label="Coldem">
      <span className="brand__mark" aria-hidden="true">
        <img src={dancoldLogo} alt="" draggable={false} />
      </span>
      {!compact && <span className="brand__word">COLDEM</span>}
    </div>
  );
}

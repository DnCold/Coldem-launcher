import runnerHead from "../assets/runner-head.png";

interface BrandMarkProps {
  compact?: boolean;
}

export function BrandMark({ compact = false }: BrandMarkProps) {
  return (
    <div className={`brand ${compact ? "brand--compact" : ""}`} aria-label="Coldem">
      <span className="brand__mark brand__mark--runner" aria-hidden="true">
        <img src={runnerHead} alt="" draggable={false} />
      </span>
      {!compact && <span className="brand__word">COLDEM</span>}
    </div>
  );
}

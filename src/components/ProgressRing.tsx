interface ProgressRingProps {
  value: number;
  size?: number;
}

export function ProgressRing({ value, size = 18 }: ProgressRingProps) {
  const normalized = Math.min(1, Math.max(0, value));
  return (
    <span
      className="progress-ring"
      style={{
        width: size,
        height: size,
        background: `conic-gradient(currentColor ${normalized * 360}deg, rgba(255,255,255,.22) 0deg)`
      }}
      aria-label={`${Math.round(normalized * 100)}%`}
    >
      <span />
    </span>
  );
}

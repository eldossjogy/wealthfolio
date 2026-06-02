interface DriftBandSliderProps {
  value: number;
  onChange: (value: number) => void;
  className?: string;
  label?: string;
  description?: string;
  compact?: boolean;
}

const MIN_DRIFT_BAND = 0.5;
const MAX_DRIFT_BAND = 10;

export function DriftBandSlider({
  value,
  onChange,
  className,
  label = "Drift tolerance",
  description,
  compact = false,
}: DriftBandSliderProps) {
  const pct = ((value - MIN_DRIFT_BAND) / (MAX_DRIFT_BAND - MIN_DRIFT_BAND)) * 100;

  return (
    <div className={className}>
      <div
        className={
          compact
            ? "mb-1.5 flex items-center justify-between gap-3"
            : "mb-2 flex items-center justify-between gap-3"
        }
      >
        <span className="text-foreground text-[12px] font-semibold">{label}</span>
        <span className="bg-muted text-foreground rounded-md px-2.5 py-1 text-[12px] font-semibold tabular-nums">
          ±{value.toFixed(1)}%
        </span>
      </div>
      {description && !compact && (
        <p className="text-muted-foreground mb-2 text-[11px] leading-relaxed">{description}</p>
      )}
      <input
        type="range"
        min={MIN_DRIFT_BAND}
        max={MAX_DRIFT_BAND}
        step={0.5}
        value={value}
        onChange={(event) => onChange(parseFloat(event.target.value))}
        className="lever-slider block w-full"
        style={{ ["--lever-pct" as string]: `${pct}%` }}
      />
      {!compact && (
        <div className="text-muted-foreground mt-2 flex justify-between text-[10px]">
          <span>Tight</span>
          <span>Standard</span>
          <span>Loose</span>
        </div>
      )}
    </div>
  );
}

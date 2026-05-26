interface SparklineProps {
  data: number[];
  /** Stroke color (CSS color string). */
  stroke: string;
  /** Optional area fill color; omit for line only. */
  fill?: string | null;
  width?: number;
  height?: number;
  className?: string;
}

/**
 * Minimal SVG sparkline. Scales the series across its own min/max so flat or
 * negative series render sensibly. Renders nothing for series shorter than 2.
 */
export function Sparkline({
  data,
  stroke,
  fill = null,
  width = 64,
  height = 24,
  className,
}: SparklineProps) {
  if (data.length < 2) {
    return <svg width={width} height={height} className={className} aria-hidden />;
  }

  const min = Math.min(...data);
  const max = Math.max(...data);
  const span = max - min || 1;
  const pad = 2;
  const usableH = height - pad * 2;

  const points = data.map((value, index) => {
    const x = (index / (data.length - 1)) * width;
    const y = pad + (1 - (value - min) / span) * usableH;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  });

  const line = `M${points.join(" L")}`;
  const area = `${line} L${width},${height} L0,${height} Z`;

  return (
    <svg width={width} height={height} className={className} aria-hidden>
      {fill && <path d={area} fill={fill} opacity={0.18} />}
      <path d={line} fill="none" stroke={stroke} strokeWidth={1.4} strokeLinejoin="round" />
    </svg>
  );
}

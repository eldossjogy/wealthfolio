export type HistoryChartMarkerVariant = "snapshot" | "buy" | "sell";
export type TradeMarkerVariant = Extract<HistoryChartMarkerVariant, "buy" | "sell">;

export interface RechartsMarkerShapeProps {
  cx?: number | string;
  cy?: number | string;
}

export interface RechartsActiveDotProps extends RechartsMarkerShapeProps {
  stroke?: string;
}

interface HistoryChartMarkerShapeProps extends RechartsMarkerShapeProps {
  variant: HistoryChartMarkerVariant;
  value?: number;
}

export function HistoryChartActiveDot({
  cx,
  cy,
  stroke = "var(--success)",
}: RechartsActiveDotProps) {
  return (
    <g transform={`translate(${cx ?? 0}, ${cy ?? 0})`} style={{ pointerEvents: "none" }}>
      <circle r={5} fill="var(--background)" />
      <circle r={3.5} fill={stroke} />
    </g>
  );
}

export function HistoryChartMarkerShape({
  cx,
  cy,
  variant,
  value = 0,
}: HistoryChartMarkerShapeProps) {
  if (variant === "snapshot") {
    return (
      <g
        className={value >= 0 ? "text-success" : "text-destructive"}
        transform={`translate(${cx ?? 0}, ${cy ?? 0})`}
        style={{ pointerEvents: "none" }}
      >
        <circle r={10} fill="currentColor" opacity={0.14} />
        <circle r={6} fill="currentColor" stroke="var(--background)" strokeWidth={1.5} />
      </g>
    );
  }

  const isBuy = variant === "buy";

  return (
    <g
      className={isBuy ? "text-success" : "text-blue-600 dark:text-blue-300"}
      style={{ pointerEvents: "none" }}
      transform={`translate(${cx ?? 0}, ${cy ?? 0})`}
    >
      <circle r={12} fill="currentColor" opacity={0.14} />
      <circle r={8} fill="currentColor" stroke="var(--background)" strokeWidth={1.5} />
      <text
        x={0}
        y={0}
        textAnchor="middle"
        dominantBaseline="central"
        fill="white"
        fontSize={10}
        fontWeight="bold"
      >
        {isBuy ? "B" : "S"}
      </text>
    </g>
  );
}

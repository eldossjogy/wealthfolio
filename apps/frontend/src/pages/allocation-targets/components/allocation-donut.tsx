import type { DriftRow } from "@/lib/types";
import { formatAmount } from "@/lib/utils";

interface AllocationDonutProps {
  rows: DriftRow[];
  totalValue: number;
  currency: string;
  size?: number;
  hoveredId?: string | null;
  onHoverChange?: (id: string | null) => void;
}

function polarToCartesian(cx: number, cy: number, r: number, angleDeg: number) {
  const rad = ((angleDeg - 90) * Math.PI) / 180;
  return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
}

function segmentPath(
  cx: number,
  cy: number,
  outerR: number,
  innerR: number,
  startDeg: number,
  endDeg: number,
): string {
  const span = endDeg - startDeg;
  if (span >= 359.99) {
    // Full circle via two semicircles
    const o0 = polarToCartesian(cx, cy, outerR, 0);
    const o180 = polarToCartesian(cx, cy, outerR, 180);
    const i0 = polarToCartesian(cx, cy, innerR, 0);
    const i180 = polarToCartesian(cx, cy, innerR, 180);
    return [
      `M ${o0.x} ${o0.y}`,
      `A ${outerR} ${outerR} 0 1 1 ${o180.x} ${o180.y}`,
      `A ${outerR} ${outerR} 0 1 1 ${o0.x} ${o0.y}`,
      `M ${i0.x} ${i0.y}`,
      `A ${innerR} ${innerR} 0 1 0 ${i180.x} ${i180.y}`,
      `A ${innerR} ${innerR} 0 1 0 ${i0.x} ${i0.y}`,
      `Z`,
    ].join(" ");
  }
  const large = span > 180 ? 1 : 0;
  const o1 = polarToCartesian(cx, cy, outerR, startDeg);
  const o2 = polarToCartesian(cx, cy, outerR, endDeg);
  const i2 = polarToCartesian(cx, cy, innerR, endDeg);
  const i1 = polarToCartesian(cx, cy, innerR, startDeg);
  return [
    `M ${o1.x} ${o1.y}`,
    `A ${outerR} ${outerR} 0 ${large} 1 ${o2.x} ${o2.y}`,
    `L ${i2.x} ${i2.y}`,
    `A ${innerR} ${innerR} 0 ${large} 0 ${i1.x} ${i1.y}`,
    `Z`,
  ].join(" ");
}

export function AllocationDonut({
  rows,
  totalValue,
  currency,
  size = 240,
  hoveredId,
  onHoverChange,
}: AllocationDonutProps) {
  const thickness = Math.round(size * 0.11);
  const outerR = size / 2 - 2;
  const innerR = outerR - thickness;
  const cx = size / 2;
  const cy = size / 2;
  const total = rows.reduce((s, r) => s + r.currentBps, 0) || 10000;

  let accDeg = 0;
  const segments = rows.map((r) => {
    const span = (r.currentBps / total) * 360;
    const start = accDeg;
    const end = accDeg + span;
    accDeg = end;
    const midDeg = start + span / 2;
    const rad = ((midDeg - 90) * Math.PI) / 180;
    return { ...r, start, end, midRad: rad };
  });

  const hoveredRow = hoveredId ? rows.find((r) => r.categoryId === hoveredId) : null;
  const popDist = 5;

  return (
    <div className="relative" style={{ width: size, height: size }}>
      <svg
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        style={{ overflow: "visible" }}
      >
        {/* Background ring */}
        <circle
          cx={cx}
          cy={cy}
          r={(outerR + innerR) / 2}
          fill="none"
          stroke="var(--muted)"
          strokeWidth={thickness}
        />
        {segments.map((s) => {
          const isHovered = hoveredId === s.categoryId;
          const dimmed = hoveredId !== null && !isHovered;
          const tx = isHovered ? (popDist * Math.cos(s.midRad)).toFixed(2) : "0";
          const ty = isHovered ? (popDist * Math.sin(s.midRad)).toFixed(2) : "0";
          return (
            <path
              key={s.categoryId}
              d={segmentPath(cx, cy, outerR, innerR, s.start, s.end)}
              fill={s.color || "var(--muted-foreground)"}
              opacity={dimmed ? 0.3 : 1}
              transform={`translate(${tx}, ${ty})`}
              style={{ transition: "opacity 0.15s ease, transform 0.12s ease", cursor: "pointer" }}
              onMouseEnter={() => onHoverChange?.(s.categoryId)}
              onMouseLeave={() => onHoverChange?.(null)}
            />
          );
        })}
      </svg>

      {/* Center label */}
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
        {hoveredRow ? (
          <>
            <div className="text-muted-foreground max-w-[75%] truncate text-[10px] uppercase tracking-wider">
              {hoveredRow.categoryName}
            </div>
            <div
              className="text-foreground mt-0.5 font-semibold tabular-nums"
              style={{ fontSize: Math.round(size * 0.1) }}
            >
              {(hoveredRow.currentBps / 100).toFixed(1)}%
            </div>
            <div
              className="text-muted-foreground mt-0.5 tabular-nums"
              style={{ fontSize: Math.round(size * 0.052) }}
            >
              {formatAmount((hoveredRow.currentBps / 10000) * totalValue, currency)}
            </div>
            {hoveredRow.status !== "in_band" && hoveredRow.status !== "not_targeted" && (
              <div
                className="mt-1 text-[10px] font-semibold uppercase tracking-wide"
                style={{
                  color: hoveredRow.status === "overweight" ? "var(--destructive)" : "#2563eb",
                }}
              >
                {hoveredRow.status === "overweight" ? "▲ Overweight" : "▼ Underweight"}
              </div>
            )}
          </>
        ) : (
          <>
            <div className="text-muted-foreground text-[10px] uppercase tracking-wider">
              Portfolio
            </div>
            <div
              className="text-foreground mt-1 font-semibold tabular-nums"
              style={{ fontSize: Math.round(size * 0.09) }}
            >
              {formatAmount(totalValue, currency)}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

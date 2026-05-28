import type { PerformanceResult } from "@/lib/types";

const numberOrNull = (value: number | null | undefined): number | null =>
  value == null || !Number.isFinite(Number(value)) ? null : Number(value);

export function performancePeriodPnl(result: PerformanceResult | null | undefined): number | null {
  if (
    !result ||
    result.mode === "notApplicable" ||
    result.mode === "symbolPriceBased" ||
    result.dataQuality.status === "noData"
  ) {
    return null;
  }

  const { attribution } = result;

  const value =
    Number(attribution.income) +
    Number(attribution.realizedPnl) +
    Number(attribution.unrealizedPnlChange) +
    Number(attribution.fxEffect) -
    Number(attribution.fees) -
    Number(attribution.taxes) +
    Number(attribution.residual);

  return Number.isFinite(value) ? value : null;
}

export function performanceHeadlineReturn(
  result: PerformanceResult | null | undefined,
): number | null {
  if (!result) return null;

  switch (result.mode) {
    case "timeWeighted":
      return numberOrNull(result.returns.twr);
    case "valueReturn":
    case "symbolPriceBased":
      return numberOrNull(result.returns.valueReturn);
    case "notApplicable":
      return null;
  }
}

import type { ActivityDetails } from "@/lib/types";

export function getMobileActivityAssetId(activity?: Partial<ActivityDetails>): string | undefined {
  return activity?.assetSymbol?.trim() || activity?.assetId?.trim() || undefined;
}

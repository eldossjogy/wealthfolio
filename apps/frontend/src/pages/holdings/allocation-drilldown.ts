import type { PortfolioAllocations, TaxonomyAllocation } from "@/lib/types";

const CUSTOM_TYPE_PREFIX = "custom:";

export function customAllocationDrilldownType(taxonomyId: string): string {
  return `${CUSTOM_TYPE_PREFIX}${taxonomyId}`;
}

export function resolveAllocationForDrilldown(
  allocations: PortfolioAllocations | undefined,
  type: string,
): TaxonomyAllocation | undefined {
  if (!allocations) return undefined;

  switch (type) {
    case "class":
      return allocations.assetClasses;
    case "sector":
      return allocations.sectors;
    case "country":
      return allocations.regions;
    case "risk":
      return allocations.riskCategory;
    case "securityType":
      return allocations.securityTypes;
    default:
      if (type.startsWith(CUSTOM_TYPE_PREFIX)) {
        const taxonomyId = type.slice(CUSTOM_TYPE_PREFIX.length);
        return allocations.customGroups.find((allocation) => allocation.taxonomyId === taxonomyId);
      }
      return undefined;
  }
}

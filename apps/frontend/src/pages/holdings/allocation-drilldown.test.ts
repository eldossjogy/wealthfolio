import { describe, expect, it } from "vitest";

import type { PortfolioAllocations, TaxonomyAllocation } from "@/lib/types";
import {
  customAllocationDrilldownType,
  resolveAllocationForDrilldown,
} from "./allocation-drilldown";

function allocation(taxonomyId: string, taxonomyName = taxonomyId): TaxonomyAllocation {
  return {
    taxonomyId,
    taxonomyName,
    color: "#000000",
    categories: [],
  };
}

describe("allocation drilldown resolver", () => {
  const allocations: PortfolioAllocations = {
    assetClasses: allocation("asset_classes"),
    sectors: allocation("industries_gics"),
    regions: allocation("regions"),
    riskCategory: allocation("risk_category"),
    securityTypes: allocation("instrument_type"),
    customGroups: [allocation("custom_groups"), allocation("custom_strategy")],
    totalValue: 100,
  };

  it("resolves built-in allocation drilldowns", () => {
    expect(resolveAllocationForDrilldown(allocations, "class")?.taxonomyId).toBe("asset_classes");
    expect(resolveAllocationForDrilldown(allocations, "sector")?.taxonomyId).toBe(
      "industries_gics",
    );
    expect(resolveAllocationForDrilldown(allocations, "country")?.taxonomyId).toBe("regions");
    expect(resolveAllocationForDrilldown(allocations, "risk")?.taxonomyId).toBe("risk_category");
    expect(resolveAllocationForDrilldown(allocations, "securityType")?.taxonomyId).toBe(
      "instrument_type",
    );
  });

  it("resolves the exact custom taxonomy instead of the first custom group", () => {
    expect(
      resolveAllocationForDrilldown(allocations, customAllocationDrilldownType("custom_strategy"))
        ?.taxonomyId,
    ).toBe("custom_strategy");
  });
});

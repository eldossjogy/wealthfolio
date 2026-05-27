export interface ModelPreset {
  id: string;
  taxonomyId: string;
  name: string;
  description: string;
  risk: string;
  expectedReturn?: number;
  volatility?: number;
  weights: Record<string, number>; // 0-100, keyed by taxonomy category id
}

export const BUILT_IN_PRESETS: ModelPreset[] = [
  // ── Asset Classes ────────────────────────────────────────────────────────────
  {
    id: "three_fund",
    taxonomyId: "asset_classes",
    name: "Three-Fund",
    description: "Bogleheads classic — US stocks, international & bonds",
    risk: "Moderate",
    expectedReturn: 7.0,
    volatility: 11.0,
    weights: { EQUITY: 60, FIXED_INCOME: 30, CASH: 10 },
  },
  {
    id: "sixty_forty",
    taxonomyId: "asset_classes",
    name: "60 / 40",
    description: "The benchmark — balanced stocks & bonds",
    risk: "Moderate",
    expectedReturn: 6.4,
    volatility: 10.1,
    weights: { EQUITY: 60, FIXED_INCOME: 40 },
  },
  {
    id: "all_weather",
    taxonomyId: "asset_classes",
    name: "All Weather",
    description: "Ray Dalio — diversified across all market regimes",
    risk: "Conservative",
    expectedReturn: 5.6,
    volatility: 7.8,
    weights: { EQUITY: 30, FIXED_INCOME: 55, COMMODITIES: 7, CASH: 8 },
  },

  // ── Industries GICS ──────────────────────────────────────────────────────────
  {
    id: "gics_sp500_weight",
    taxonomyId: "industries_gics",
    name: "S&P 500",
    description: "Approximate S&P 500 sector weights",
    risk: "Moderate",
    expectedReturn: 7.5,
    volatility: 15.0,
    weights: {
      "45": 31,
      "40": 13,
      "35": 12,
      "25": 10,
      "50": 9,
      "20": 8,
      "30": 6,
      "10": 4,
      "55": 3,
      "60": 2,
      "15": 2,
    },
  },
  {
    id: "gics_equal_weight",
    taxonomyId: "industries_gics",
    name: "Equal Weight",
    description: "Equal allocation across all 11 GICS sectors",
    risk: "Moderate",
    expectedReturn: 6.8,
    volatility: 14.0,
    weights: {
      "10": 9,
      "15": 9,
      "20": 9,
      "25": 9,
      "30": 9,
      "35": 9,
      "40": 9,
      "45": 10,
      "50": 9,
      "55": 9,
      "60": 9,
    },
  },
  {
    id: "gics_defensive",
    taxonomyId: "industries_gics",
    name: "Defensive",
    description: "Overweight staples, health care & utilities",
    risk: "Conservative",
    expectedReturn: 5.2,
    volatility: 10.5,
    weights: { "30": 25, "35": 25, "55": 20, "40": 10, "10": 10, "20": 10 },
  },

  // ── Regions ──────────────────────────────────────────────────────────────────
  {
    id: "regions_global_cap",
    taxonomyId: "regions",
    name: "Global Cap",
    description: "Approximate world market-cap weights",
    risk: "Moderate",
    expectedReturn: 7.0,
    volatility: 13.5,
    weights: { R20: 63, R10: 17, R30: 17, R50: 2, R40: 1 },
  },
  {
    id: "regions_ex_us",
    taxonomyId: "regions",
    name: "Ex-US",
    description: "International focus — minimize Americas exposure",
    risk: "Moderate",
    expectedReturn: 6.8,
    volatility: 14.5,
    weights: { R10: 42, R30: 38, R20: 10, R50: 8, R40: 2 },
  },
  {
    id: "regions_equal_weight",
    taxonomyId: "regions",
    name: "Equal Weight",
    description: "Equal exposure across all five continents",
    risk: "Aggressive",
    expectedReturn: 7.2,
    volatility: 16.0,
    weights: { R10: 20, R20: 20, R30: 20, R40: 20, R50: 20 },
  },
];

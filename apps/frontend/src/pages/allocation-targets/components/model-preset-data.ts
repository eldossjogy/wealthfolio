export interface ModelPreset {
  id: string;
  name: string;
  description: string;
  risk: string;
  expectedReturn?: number;
  volatility?: number;
  weights: Record<string, number>; // 0-100, keyed by asset_classes category key
}

export const BUILT_IN_PRESETS: ModelPreset[] = [
  {
    id: "three_fund",
    name: "Three-Fund",
    description: "Bogleheads classic — US stocks, international & bonds",
    risk: "Moderate",
    expectedReturn: 7.0,
    volatility: 11.0,
    weights: { EQUITY: 60, FIXED_INCOME: 30, CASH: 10 },
  },
  {
    id: "sixty_forty",
    name: "60 / 40",
    description: "The benchmark — balanced stocks & bonds",
    risk: "Moderate",
    expectedReturn: 6.4,
    volatility: 10.1,
    weights: { EQUITY: 60, FIXED_INCOME: 40 },
  },
  {
    id: "all_weather",
    name: "All Weather",
    description: "Ray Dalio — diversified across all market regimes",
    risk: "Conservative",
    expectedReturn: 5.6,
    volatility: 7.8,
    weights: { EQUITY: 30, FIXED_INCOME: 55, COMMODITIES: 7, CASH: 8 },
  },
];

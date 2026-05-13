# E2e Market Data Fixtures

The e2e fixture provider reads `instruments.json` and generates deterministic
synthetic OHLCV quotes at runtime. No real market-data rows are stored here.

- `instruments.json`: synthetic provider metadata for search, profile, quote
  currency, and quote generation.
- `WEALTHFOLIO_FIXTURE_AS_OF`: optional latest-quote date. Defaults to
  `2026-05-12`; set to `today` for local exploratory runs.

Search results, profiles, latest quotes, historical quotes, and splits all use
the same instrument metadata, so currencies and symbols stay consistent.

FX pairs found in e2e runs are explicitly listed for the CAD, USD, EUR, and GBP
lanes. The provider also generates deterministic FX quotes for missing
Yahoo-shaped or slash-shaped pairs such as `CADUSD=X` or `CAD/USD`.

Custom/manual symbols and expected invalid mappings are intentionally not
listed: `MYASSET`, `MYCOIN`, `TESTASSET01`, `INVALID_TICKER_XYZ_E2E`, and
`INVALID_BF_XYZ_E2E`.

#!/usr/bin/env bash
# Apply scripts/repaint-spending-colors.sql to the local Wealthfolio DB.
# Backs up the db first to <db>.bak-YYYYMMDD-HHMMSS.
#
# Usage:
#   ./scripts/repaint-spending-colors.sh                   # default macOS path
#   ./scripts/repaint-spending-colors.sh /path/to/app.db   # custom path

set -euo pipefail

DEFAULT_DB="$HOME/Library/Application Support/com.teymz.wealthfolio/app.db"
DB="${1:-$DEFAULT_DB}"
SQL="$(dirname "$0")/repaint-spending-colors.sql"

if [[ ! -f "$DB" ]]; then
  echo "❌ DB not found: $DB" >&2
  echo "   Pass a custom path as the first argument." >&2
  exit 1
fi

if [[ ! -f "$SQL" ]]; then
  echo "❌ SQL script not found: $SQL" >&2
  exit 1
fi

# Refuse to run if the app is holding a write lock (WAL file actively in use
# usually means SQLite has open handles — close Wealthfolio first).
if lsof -- "$DB" >/dev/null 2>&1; then
  echo "⚠️  $DB appears to be in use. Quit Wealthfolio first, then re-run."
  exit 1
fi

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$DB.bak-$STAMP"
cp "$DB" "$BACKUP"
echo "📦 Backed up to: $BACKUP"

echo "🎨 Applying color repaint…"
sqlite3 "$DB" < "$SQL"

echo "✅ Done. Restart Wealthfolio to see the new colors."

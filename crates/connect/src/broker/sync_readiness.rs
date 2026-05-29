use chrono::{DateTime, NaiveDate, Utc};

use super::models::BrokerSyncStatusDetail;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderReadiness {
    Ready(NaiveDate),
    NotReady(String),
}

pub fn parse_provider_sync_date(value: &str) -> Result<NaiveDate, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc).date_naive())
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .map_err(|_| format!("Invalid provider sync timestamp '{}'", value))
}

pub fn resolve_activity_readiness(
    status: Option<&BrokerSyncStatusDetail>,
) -> Result<ProviderReadiness, String> {
    let Some(status) = status else {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync status is unavailable".to_string(),
        ));
    };

    if status.initial_sync_completed == Some(false) {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync is still preparing".to_string(),
        ));
    }

    let Some(last_successful_sync) = status.last_successful_sync.as_deref() else {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync has no successful waterline".to_string(),
        ));
    };

    Ok(ProviderReadiness::Ready(parse_provider_sync_date(
        last_successful_sync,
    )?))
}

pub fn resolve_holdings_readiness(
    status: Option<&BrokerSyncStatusDetail>,
) -> Result<ProviderReadiness, String> {
    let Some(status) = status else {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync status is unavailable".to_string(),
        ));
    };

    if status.initial_sync_completed == Some(false) {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync is still preparing".to_string(),
        ));
    }

    let Some(last_successful_sync) = status.last_successful_sync.as_deref() else {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync has no successful waterline".to_string(),
        ));
    };

    Ok(ProviderReadiness::Ready(parse_provider_sync_date(
        last_successful_sync,
    )?))
}

pub fn provider_waterline_precedes_local_cursor(
    local_cursor: Option<&DateTime<Utc>>,
    provider_waterline: NaiveDate,
) -> bool {
    local_cursor
        .map(|cursor| provider_waterline < cursor.date_naive())
        .unwrap_or(false)
}

pub fn should_advance_activity_cursor(
    fetched: usize,
    has_local_cursor: bool,
    inconsistent_empty_page: bool,
    provider_status: Option<&BrokerSyncStatusDetail>,
) -> bool {
    if inconsistent_empty_page {
        return false;
    }

    if fetched > 0 || has_local_cursor {
        return true;
    }

    matches!(
        provider_status,
        Some(BrokerSyncStatusDetail {
            initial_sync_completed: Some(true),
            first_transaction_date: None,
            ..
        })
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transactions_status(
        initial_sync_completed: Option<bool>,
        last_successful_sync: Option<&str>,
        first_transaction_date: Option<&str>,
    ) -> BrokerSyncStatusDetail {
        BrokerSyncStatusDetail {
            initial_sync_completed,
            last_successful_sync: last_successful_sync.map(str::to_string),
            first_transaction_date: first_transaction_date.map(str::to_string),
        }
    }

    #[test]
    fn provider_not_ready_defers_first_activity_sync() {
        let status = transactions_status(Some(false), None, None);
        let readiness = resolve_activity_readiness(Some(&status)).unwrap();

        assert!(matches!(readiness, ProviderReadiness::NotReady(_)));
    }

    #[test]
    fn provider_waterline_without_initial_flag_allows_activity_fetch() {
        let status = transactions_status(None, Some("2026-05-22"), None);
        let readiness = resolve_activity_readiness(Some(&status)).unwrap();

        assert!(matches!(
            readiness,
            ProviderReadiness::Ready(date)
                if date == NaiveDate::from_ymd_opt(2026, 5, 22).unwrap()
        ));
    }

    #[test]
    fn provider_waterline_before_local_cursor_is_stale() {
        let local_cursor = DateTime::parse_from_rfc3339("2026-05-22T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let stale_waterline = NaiveDate::from_ymd_opt(2026, 5, 21).unwrap();
        let current_waterline = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();

        assert!(provider_waterline_precedes_local_cursor(
            Some(&local_cursor),
            stale_waterline
        ));
        assert!(!provider_waterline_precedes_local_cursor(
            Some(&local_cursor),
            current_waterline
        ));
    }

    #[test]
    fn initial_empty_activity_sync_with_first_transaction_does_not_advance_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), Some("2020-01-01"));

        assert!(!should_advance_activity_cursor(
            0,
            false,
            false,
            Some(&status)
        ));
    }

    #[test]
    fn initial_empty_activity_sync_with_confirmed_empty_history_advances_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), None);

        assert!(should_advance_activity_cursor(
            0,
            false,
            false,
            Some(&status)
        ));
    }

    #[test]
    fn incremental_empty_activity_sync_advances_cursor_to_provider_waterline() {
        let status = transactions_status(Some(true), Some("2026-05-22"), Some("2020-01-01"));

        assert!(should_advance_activity_cursor(
            0,
            true,
            false,
            Some(&status)
        ));
    }

    #[test]
    fn inconsistent_empty_activity_page_does_not_advance_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), None);

        assert!(!should_advance_activity_cursor(
            0,
            true,
            true,
            Some(&status)
        ));
    }

    #[test]
    fn holdings_not_ready_defers_snapshot_sync() {
        let status = transactions_status(Some(false), None, None);
        let readiness = resolve_holdings_readiness(Some(&status)).unwrap();

        assert!(matches!(readiness, ProviderReadiness::NotReady(_)));
    }
}

use std::time::Duration;

use chrono::{DateTime, Utc};
use opentk_sync::runner::{
    skiptoken_from_url, CategoryStatus, CategorySyncState, DurableSyncError, PreparedSyncPage,
    StoredCategoryCursor, SyncPhase, SyncStore, SyncStoreError, SyncStoreFuture,
    SyncStoreWriteOutcome,
};
use reqwest::Url;
use sqlx::{PgPool, Row};

use crate::sync_writer::{write_sync_page, SyncPageWrite};

#[derive(Clone, Debug)]
pub struct PostgresSyncStore {
    pool: PgPool,
}

impl PostgresSyncStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl SyncStore for PostgresSyncStore {
    fn load_category_cursor<'a>(
        &'a self,
        category: &'a str,
    ) -> SyncStoreFuture<'a, Option<StoredCategoryCursor>> {
        Box::pin(async move {
            let row = sqlx::query(
                r"
                SELECT source_category, latest_skiptoken, next_url, state
                FROM sync_category
                WHERE source_category = $1
                ",
            )
            .bind(category)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| store_error(&error))?;
            let Some(row) = row else {
                return Ok(None);
            };
            let next_url: String = row
                .try_get("next_url")
                .map_err(|error| store_error(&error))?;
            let state: String = row.try_get("state").map_err(|error| store_error(&error))?;
            Ok(Some(StoredCategoryCursor {
                category: row
                    .try_get("source_category")
                    .map_err(|error| store_error(&error))?,
                latest_skiptoken: row
                    .try_get("latest_skiptoken")
                    .map_err(|error| store_error(&error))?,
                next_url: Url::parse(&next_url).map_err(|error| store_error(&error))?,
                caught_up: CategorySyncState::parse(&state)? == CategorySyncState::CaughtUp,
            }))
        })
    }

    fn write_page(&self, page: PreparedSyncPage) -> SyncStoreFuture<'_, SyncStoreWriteOutcome> {
        Box::pin(async move {
            let outcome = write_sync_page(
                &self.pool,
                SyncPageWrite {
                    category: page.category,
                    latest_skiptoken: page.latest_skiptoken,
                    next_url: Some(page.next_url.to_string()),
                    atom_updated_at: page.atom_updated_at,
                    entities: page.entities,
                },
            )
            .await
            .map_err(|error| store_error(&error))?;
            Ok(SyncStoreWriteOutcome {
                entities_seen: outcome.entities_seen,
                entities_written: outcome.entities_written,
                entities_deleted: outcome.entities_deleted,
            })
        })
    }

    fn mark_caught_up<'a>(
        &'a self,
        category: &'a str,
        resume_url: Url,
        observed_at: DateTime<Utc>,
    ) -> SyncStoreFuture<'a, ()> {
        Box::pin(async move {
            let latest_skiptoken =
                skiptoken_from_url(&resume_url).ok_or_else(|| SyncStoreError {
                    message: format!("resume URL for {category} does not contain skiptoken"),
                })?;
            sqlx::query(
                r"
                INSERT INTO sync_category (
                    source_category,
                    latest_skiptoken,
                    last_synced_at,
                    next_url,
                    resume_url,
                    state,
                    last_fetch_at,
                    caught_up_at
                )
                VALUES ($1, $2, $3, $4, $4, $5, $3, $3)
                ON CONFLICT (source_category) DO UPDATE
                SET latest_skiptoken = EXCLUDED.latest_skiptoken,
                    last_synced_at = EXCLUDED.last_synced_at,
                    next_url = EXCLUDED.next_url,
                    resume_url = EXCLUDED.resume_url,
                    state = EXCLUDED.state,
                    last_fetch_at = EXCLUDED.last_fetch_at,
                    caught_up_at = EXCLUDED.caught_up_at
                ",
            )
            .bind(category)
            .bind(latest_skiptoken)
            .bind(observed_at)
            .bind(resume_url.to_string())
            .bind(CategorySyncState::CaughtUp.as_str())
            .execute(&self.pool)
            .await
            .map_err(|error| store_error(&error))?;
            Ok(())
        })
    }

    fn record_error(&self, error: DurableSyncError) -> SyncStoreFuture<'_, ()> {
        Box::pin(async move {
            sqlx::query(
                r"
                INSERT INTO ingest_error (
                    phase,
                    source_category,
                    source_id,
                    latest_skiptoken,
                    message,
                    payload,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, NULL, $6)
                ",
            )
            .bind(error.phase.as_str())
            .bind(&error.category)
            .bind(error.entity_id)
            .bind(error.skiptoken)
            .bind(&error.message)
            .bind(Utc::now())
            .execute(&self.pool)
            .await
            .map_err(|error| store_error(&error))?;
            Ok(())
        })
    }

    fn status<'a>(&'a self, categories: &'a [String]) -> SyncStoreFuture<'a, Vec<CategoryStatus>> {
        Box::pin(async move {
            let mut statuses = Vec::with_capacity(categories.len());
            for category in categories {
                let progress = sqlx::query(
                    r"
                    SELECT latest_skiptoken, state, last_fetch_at
                    FROM sync_category
                    WHERE source_category = $1
                    ",
                )
                .bind(category)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| store_error(&error))?;
                let (latest_skiptoken, state, last_fetch_at) = if let Some(progress) = progress {
                    let state_text: String = progress
                        .try_get("state")
                        .map_err(|error| store_error(&error))?;
                    (
                        Some(
                            progress
                                .try_get("latest_skiptoken")
                                .map_err(|error| store_error(&error))?,
                        ),
                        CategorySyncState::parse(&state_text)?,
                        progress
                            .try_get("last_fetch_at")
                            .map_err(|error| store_error(&error))?,
                    )
                } else {
                    (None, CategorySyncState::NotStarted, None)
                };
                let lag = match last_fetch_at {
                    Some(last_fetch_at) => Some(lag_since(last_fetch_at)?),
                    None => None,
                };
                let error = load_last_error(&self.pool, category).await?;
                let error = error.and_then(|(error, created_at)| {
                    (!is_superseded_error(last_fetch_at, created_at)).then_some(error)
                });
                statuses.push(CategoryStatus {
                    category: category.clone(),
                    latest_skiptoken,
                    state: if error.is_some() {
                        CategorySyncState::Error
                    } else {
                        state
                    },
                    lag,
                    last_fetch_at,
                    last_error: error,
                });
            }
            Ok(statuses)
        })
    }
}

async fn load_last_error(
    pool: &PgPool,
    category: &str,
) -> Result<Option<(DurableSyncError, DateTime<Utc>)>, SyncStoreError> {
    let row = sqlx::query(
        r"
        SELECT phase, source_category, source_id, latest_skiptoken, message, created_at
        FROM ingest_error
        WHERE source_category = $1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        ",
    )
    .bind(category)
    .fetch_optional(pool)
    .await
    .map_err(|error| store_error(&error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let phase: String = row.try_get("phase").map_err(|error| store_error(&error))?;
    Ok(Some((
        DurableSyncError {
            phase: SyncPhase::parse(&phase)?,
            category: row
                .try_get("source_category")
                .map_err(|error| store_error(&error))?,
            entity_id: row
                .try_get("source_id")
                .map_err(|error| store_error(&error))?,
            skiptoken: row
                .try_get("latest_skiptoken")
                .map_err(|error| store_error(&error))?,
            message: row
                .try_get("message")
                .map_err(|error| store_error(&error))?,
        },
        row.try_get("created_at")
            .map_err(|error| store_error(&error))?,
    )))
}

fn lag_since(last_fetch_at: DateTime<Utc>) -> Result<Duration, SyncStoreError> {
    (Utc::now() - last_fetch_at)
        .to_std()
        .map_err(|_| SyncStoreError {
            message: format!(
                "sync last_fetch_at {last_fetch_at} is in the future; lag cannot be negative"
            ),
        })
}

fn store_error(error: &impl ToString) -> SyncStoreError {
    SyncStoreError {
        message: error.to_string(),
    }
}

fn is_superseded_error(
    last_fetch_at: Option<DateTime<Utc>>,
    error_created_at: DateTime<Utc>,
) -> bool {
    last_fetch_at.is_some_and(|last_fetch_at| last_fetch_at >= error_created_at)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::is_superseded_error;

    #[test]
    fn newer_progress_supersedes_historical_error() {
        let error_created_at = Utc::now();
        let last_fetch_at = error_created_at + Duration::seconds(30);

        assert!(is_superseded_error(Some(last_fetch_at), error_created_at));
        assert!(!is_superseded_error(
            Some(error_created_at - Duration::seconds(30)),
            error_created_at
        ));
        assert!(!is_superseded_error(None, error_created_at));
    }
}

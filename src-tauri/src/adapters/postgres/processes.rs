use deadpool_postgres::Pool;

use crate::error::{Error, Result};
use crate::models::DatabaseProcess;

/// Reads the activity view from a separate pooled connection. `pg_stat_activity`
/// is server-wide, so the selected database only determines which credentials
/// are used to access the view.
pub async fn list(pool: &Pool) -> Result<Vec<DatabaseProcess>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                a.pid,
                a.datname,
                a.usename,
                NULLIF(a.application_name, ''),
                a.client_addr::text,
                a.client_port,
                a.state,
                a.query,
                a.query_start::text,
                a.xact_start::text,
                a.backend_start::text,
                a.wait_event_type,
                a.wait_event,
                a.backend_type,
                CASE
                    WHEN a.state = 'active' AND a.query_start IS NOT NULL
                    THEN EXTRACT(EPOCH FROM (clock_timestamp() - a.query_start) * 1000)::bigint
                    ELSE NULL
                END,
                a.pid = pg_backend_pid()
            FROM pg_stat_activity a
            ORDER BY
                CASE WHEN a.state = 'active' THEN 0 ELSE 1 END,
                a.query_start NULLS LAST,
                a.pid
            "#,
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| DatabaseProcess {
            pid: row.get(0),
            database: row.get(1),
            username: row.get(2),
            application_name: row.get(3),
            client_address: row.get(4),
            client_port: row.get(5),
            state: row.get(6),
            query: row.get(7),
            query_start: row.get(8),
            transaction_start: row.get(9),
            backend_start: row.get(10),
            wait_event_type: row.get(11),
            wait_event: row.get(12),
            backend_type: row.get(13),
            duration_ms: row.get(14),
            is_own_process: row.get(15),
        })
        .collect())
}

/// Terminates a backend through another pooled connection. PostgreSQL returns
/// false when the PID is gone or the current role is not allowed to signal it.
pub async fn terminate(pool: &Pool, pid: i32) -> Result<bool> {
    if pid <= 0 {
        return Err(Error::InvalidQuery("Process PID must be positive".into()));
    }

    let client = pool.get().await?;
    let own_pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);

    if pid == own_pid {
        return Err(Error::InvalidQuery(
            "The monitor cannot terminate its own connection".into(),
        ));
    }

    let terminated: bool = client
        .query_one("SELECT pg_terminate_backend($1::int)", &[&pid])
        .await?
        .get(0);

    if !terminated {
        return Err(Error::Query(format!(
            "Could not terminate backend {pid}; it may have ended or the role lacks permission"
        )));
    }

    Ok(true)
}

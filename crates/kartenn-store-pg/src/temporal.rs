//! Dated reads on versioned tables (REQ-004-17, 18; ADR 0017).
//!
//! A versioned table `t` has a history twin `t_history` (migration 0002). A read at
//! `(valid_at, as_of)` takes, from the versions asserted at `as_of`, those valid at
//! `valid_at`: when `as_of` is now, the current table alone; otherwise the current rows
//! already asserted at `as_of` plus the history rows asserted then.
//!
//! The builders are the SQL audit point: names are validated identifiers from the code,
//! values are always bound parameters; hence they return [`AssertSqlSafe`].

use sqlx::AssertSqlSafe;

/// Errors of the query builders.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TemporalError {
    /// A table or column name is not a plain lowercase identifier.
    #[error("{0:?} is not a plain SQL identifier")]
    Identifier(String),
}

fn identifier(name: &str) -> Result<&str, TemporalError> {
    let mut bytes = name.bytes();
    let valid = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if valid {
        Ok(name)
    } else {
        Err(TemporalError::Identifier(name.to_owned()))
    }
}

fn column_list(columns: &[&str]) -> Result<String, TemporalError> {
    Ok(columns
        .iter()
        .map(|c| identifier(c))
        .collect::<Result<Vec<_>, _>>()?
        .join(", "))
}

/// Which system time a read uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsOf {
    /// The current assertions: current table only.
    Now,
    /// A past system time, bound as parameter `$2`.
    Past,
}

/// `SELECT columns` of `table` valid at `$1` (`timestamptz`) and asserted now or at `$2`
/// (`timestamptz`), plus `extra_where` (parameterised by the caller from `$3`), applied to
/// both branches.
///
/// # Errors
///
/// [`TemporalError::Identifier`] for an invalid table or column name.
pub fn select_at(
    table: &str,
    columns: &[&str],
    as_of: AsOf,
    extra_where: &str,
) -> Result<AssertSqlSafe<String>, TemporalError> {
    let table = identifier(table)?;
    let cols = column_list(columns)?;
    let extra = if extra_where.is_empty() {
        String::new()
    } else {
        format!(" AND ({extra_where})")
    };
    let sql = match as_of {
        AsOf::Now => {
            format!("SELECT {cols} FROM {table} WHERE valid_period @> $1::timestamptz{extra}")
        }
        AsOf::Past => format!(
            "SELECT {cols} FROM {table} \
             WHERE valid_period @> $1::timestamptz AND sys_start <= $2::timestamptz{extra} \
             UNION ALL \
             SELECT {cols} FROM {table}_history \
             WHERE valid_period @> $1::timestamptz AND sys_start <= $2::timestamptz \
             AND sys_end > $2::timestamptz{extra}"
        ),
    };
    Ok(AssertSqlSafe(sql))
}

/// A relation (to use in `FROM`) of the rows of `table` valid at `$1` and asserted at `$2`
/// (`timestamptz`; null for the current assertions), with `columns`.
///
/// # Errors
///
/// [`TemporalError::Identifier`] for an invalid table or column name.
pub fn snapshot(table: &str, columns: &[&str]) -> Result<String, TemporalError> {
    let table = identifier(table)?;
    let cols = column_list(columns)?;
    Ok(format!(
        "(SELECT {cols} FROM {table} \
          WHERE valid_period @> $1::timestamptz \
            AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz) \
          UNION ALL \
          SELECT {cols} FROM {table}_history \
          WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz \
            AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    ))
}

/// Every version of the object `$1` of `table`, on both axes, with the changeset that
/// produced it: `columns`, `valid_period`, `sys_start`, `sys_end` (null when current),
/// `changeset_id`, ordered by application then system time (REQ-004-18).
///
/// # Errors
///
/// [`TemporalError::Identifier`] for an invalid table or column name.
pub fn history_of(table: &str, columns: &[&str]) -> Result<AssertSqlSafe<String>, TemporalError> {
    let table = identifier(table)?;
    let cols = column_list(columns)?;
    Ok(AssertSqlSafe(format!(
        "SELECT {cols}, valid_period, sys_start, NULL::timestamptz AS sys_end, changeset_id \
         FROM {table} WHERE id = $1 \
         UNION ALL \
         SELECT {cols}, valid_period, sys_start, sys_end, changeset_id FROM {table}_history WHERE id = $1 \
         ORDER BY valid_period, sys_start"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_checked() {
        assert!(select_at("address", &["id", "usage"], AsOf::Now, "").is_ok());
        assert_eq!(
            select_at("address; DROP TABLE x", &["id"], AsOf::Now, "").map(|s| s.0),
            Err(TemporalError::Identifier("address; DROP TABLE x".into()))
        );
        assert!(select_at("address", &["id\""], AsOf::Past, "").is_err());
        assert!(history_of("Address", &["id"]).is_err());
    }

    #[test]
    fn current_reads_do_not_touch_history() {
        let sql = select_at("address", &["id"], AsOf::Now, "domain_id = $3")
            .unwrap()
            .0;
        assert!(!sql.contains("_history"));
        assert!(sql.ends_with("AND (domain_id = $3)"));
        let past = select_at("address", &["id"], AsOf::Past, "").unwrap().0;
        assert!(past.contains("address_history") && past.contains("sys_end > $2"));
    }
}

//! Proves `api/schema.sql` is structurally identical to a database produced by
//! running the migration sequence.
//!
//! Two throwaway databases are built — one via the sqlx migrator, one by
//! executing the reference file directly — then both are introspected through
//! `pg_catalog` and the resulting descriptions compared. SQL comments vanish at
//! parse time, so the reference stays readable (logical ordering, comments
//! explaining intent) without any of that participating in the comparison.
//!
//! Coverage is limited to what the migrations actually create: tables with
//! columns, constraints, indexes, enum types, and domains. There are no
//! functions, triggers, or extensions today; if a migration ever introduces
//! one, this introspection won't catch its absence from the reference and must
//! be extended.
//!
//! Column ordinal position is deliberately ignored. `ALTER TABLE ADD COLUMN`
//! appends, so comparing order would force the reference into chronological
//! rather than logical column ordering, defeating its purpose as a reference.

use std::collections::{BTreeMap, BTreeSet};

use sqlx::{PgPool, Row};

#[tokio::test]
async fn schema_reference_matches_migrations() -> anyhow::Result<()> {
    let (migrated, _) = test_helpers::setup_database().await?;

    let (reference, _) = test_helpers::setup_empty_database().await?;
    sqlx::raw_sql(include_str!("../../schema.sql"))
        .execute(&reference)
        .await?;

    let migrated = describe_schema(&migrated).await?;
    let reference = describe_schema(&reference).await?;

    let mut drift = Vec::new();
    diff_maps(
        "table",
        &migrated.tables,
        &reference.tables,
        &mut drift,
        |name, from_migrations, from_reference, drift| {
            diff_maps(
                &format!("{name} column"),
                &from_migrations.columns,
                &from_reference.columns,
                drift,
                report_value_drift,
            );
            diff_maps(
                &format!("{name} constraint"),
                &from_migrations.constraints,
                &from_reference.constraints,
                drift,
                report_value_drift,
            );
        },
    );
    diff_maps(
        "index",
        &migrated.indexes,
        &reference.indexes,
        &mut drift,
        report_value_drift,
    );
    diff_maps(
        "enum",
        &migrated.enums,
        &reference.enums,
        &mut drift,
        report_value_drift,
    );
    diff_maps(
        "domain",
        &migrated.domains,
        &reference.domains,
        &mut drift,
        report_value_drift,
    );

    assert!(
        drift.is_empty(),
        "api/schema.sql has drifted from api/migrations. Update the reference \
         so it matches what the migrations produce.\n\n{}",
        drift.join("\n"),
    );
    Ok(())
}

/// The `_sqlx_migrations` bookkeeping table exists only on the migrated side.
const SKIPPED_TABLES: [&str; 1] = ["_sqlx_migrations"];

#[derive(Debug)]
struct SchemaDescription {
    tables: BTreeMap<String, Table>,
    /// Excludes indexes backing a PK/UNIQUE constraint; those are compared as
    /// constraints, where `pg_get_constraintdef` already describes them.
    indexes: BTreeMap<String, String>,
    /// Enum name to its labels, in declaration order (order is significant:
    /// it defines the type's sort order).
    enums: BTreeMap<String, String>,
    domains: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
struct Table {
    /// Column name to type, nullability, default, identity, and generated
    /// expression.
    columns: BTreeMap<String, String>,
    /// Constraint name to `pg_get_constraintdef` output.
    constraints: BTreeMap<String, String>,
}

async fn describe_schema(pool: &PgPool) -> anyhow::Result<SchemaDescription> {
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();

    // `format_type` renders the declared type canonically (including
    // typmod, e.g. `character varying(50)`), and `pg_get_expr` renders
    // defaults and generation expressions in their parsed-and-deparsed form.
    // Both make formatting differences between the reference SQL and the
    // migration SQL (case, whitespace, implicit casts) invisible here.
    let rows = sqlx::query(
        "SELECT c.relname AS table_name,
                a.attname AS column_name,
                format_type(a.atttypid, a.atttypmod) AS data_type,
                a.attnotnull AS not_null,
                a.attidentity::text AS identity,
                pg_get_expr(d.adbin, d.adrelid) AS default_expr,
                a.attgenerated::text AS generated
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         JOIN pg_attribute a ON a.attrelid = c.oid
         LEFT JOIN pg_attrdef d
             ON d.adrelid = c.oid AND d.adnum = a.attnum
         WHERE n.nspname = 'public'
           AND c.relkind = 'r'
           AND a.attnum > 0
           AND NOT a.attisdropped",
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        let table_name: String = row.get("table_name");
        if SKIPPED_TABLES.contains(&table_name.as_str()) {
            continue;
        }
        let column_name: String = row.get("column_name");

        let data_type: String = row.get("data_type");
        let not_null: bool = row.get("not_null");
        let identity: String = row.get("identity");
        let default_expr: Option<String> = row.get("default_expr");
        let generated: String = row.get("generated");

        // `attgenerated` is '' for an ordinary column and 's' for a stored
        // generated column, where `pg_attrdef` holds the generation
        // expression rather than a default.
        let mut description = data_type;
        if not_null {
            description.push_str(" NOT NULL");
        }
        match (generated.as_str(), &default_expr) {
            ("s", Some(expr)) => {
                description
                    .push_str(&format!(" GENERATED ALWAYS AS ({expr}) STORED"));
            }
            (_, Some(expr)) => {
                description.push_str(&format!(" DEFAULT {expr}"))
            }
            (_, None) => {}
        }
        if !identity.is_empty() {
            description.push_str(&format!(" IDENTITY {identity}"));
        }

        tables
            .entry(table_name)
            .or_default()
            .columns
            .insert(column_name, description);
    }

    let rows = sqlx::query(
        "SELECT c.relname AS table_name,
                con.conname AS constraint_name,
                pg_get_constraintdef(con.oid) AS definition
         FROM pg_constraint con
         JOIN pg_class c ON c.oid = con.conrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'",
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        let table_name: String = row.get("table_name");
        if SKIPPED_TABLES.contains(&table_name.as_str()) {
            continue;
        }
        tables.entry(table_name).or_default().constraints.insert(
            row.get::<String, _>("constraint_name"),
            row.get::<String, _>("definition"),
        );
    }

    // `conindid` links a PK/UNIQUE constraint to the index implementing it;
    // excluding those here keeps each such object compared exactly once.
    let rows = sqlx::query(
        "SELECT i.relname AS index_name,
                pg_get_indexdef(i.oid) AS definition
         FROM pg_class i
         JOIN pg_namespace n ON n.oid = i.relnamespace
         JOIN pg_index idx ON idx.indexrelid = i.oid
         JOIN pg_class t ON t.oid = idx.indrelid
         WHERE n.nspname = 'public'
           AND i.relkind = 'i'
           AND t.relname <> ALL($1)
           AND NOT EXISTS (
               SELECT 1 FROM pg_constraint con
               WHERE con.conindid = i.oid
           )",
    )
    .bind(SKIPPED_TABLES)
    .fetch_all(pool)
    .await?;

    let indexes = rows
        .into_iter()
        .map(|row| (row.get("index_name"), row.get("definition")))
        .collect();

    let rows = sqlx::query(
        "SELECT t.typname AS enum_name,
                string_agg(e.enumlabel, ', ' ORDER BY e.enumsortorder)
                    AS labels
         FROM pg_type t
         JOIN pg_namespace n ON n.oid = t.typnamespace
         JOIN pg_enum e ON e.enumtypid = t.oid
         WHERE n.nspname = 'public'
         GROUP BY t.typname",
    )
    .fetch_all(pool)
    .await?;

    let enums = rows
        .into_iter()
        .map(|row| (row.get("enum_name"), row.get("labels")))
        .collect();

    let rows = sqlx::query(
        "SELECT t.typname AS domain_name,
                format_type(t.typbasetype, t.typtypmod) AS base_type,
                t.typnotnull AS not_null,
                coalesce(
                    string_agg(
                        pg_get_constraintdef(con.oid), ', ' ORDER BY con.conname
                    ),
                    ''
                ) AS constraints
         FROM pg_type t
         JOIN pg_namespace n ON n.oid = t.typnamespace
         LEFT JOIN pg_constraint con ON con.contypid = t.oid
         WHERE n.nspname = 'public'
           AND t.typtype = 'd'
         GROUP BY t.typname, t.typbasetype, t.typtypmod, t.typnotnull",
    )
    .fetch_all(pool)
    .await?;

    let domains = rows
        .into_iter()
        .map(|row| {
            let mut description: String = row.get("base_type");
            if row.get::<bool, _>("not_null") {
                description.push_str(" NOT NULL");
            }
            let constraints: String = row.get("constraints");
            if !constraints.is_empty() {
                description.push(' ');
                description.push_str(&constraints);
            }
            (row.get("domain_name"), description)
        })
        .collect();

    Ok(SchemaDescription {
        tables,
        indexes,
        enums,
        domains,
    })
}

/// Walk the union of keys in both maps, reporting objects present on only one
/// side and delegating shared keys to `compare` so failures name the exact
/// drifted object rather than dumping two whole schemas.
fn diff_maps<T>(
    kind: &str,
    from_migrations: &BTreeMap<String, T>,
    from_reference: &BTreeMap<String, T>,
    drift: &mut Vec<String>,
    mut compare: impl FnMut(&str, &T, &T, &mut Vec<String>),
) {
    let names: BTreeSet<&String> = from_migrations
        .keys()
        .chain(from_reference.keys())
        .collect();

    for name in names {
        match (from_migrations.get(name), from_reference.get(name)) {
            (Some(migrated), Some(reference)) => {
                compare(name, migrated, reference, drift)
            }
            (Some(_), None) => drift.push(format!(
                "{kind} {name}: created by migrations, missing from schema.sql"
            )),
            (None, Some(_)) => drift.push(format!(
                "{kind} {name}: defined in schema.sql, not created by migrations"
            )),
            (None, None) => unreachable!("name came from one of the maps"),
        }
    }
}

fn report_value_drift(
    name: &str,
    from_migrations: &String,
    from_reference: &String,
    drift: &mut Vec<String>,
) {
    if from_migrations != from_reference {
        drift.push(format!(
            "{name}:\n  migrations: {from_migrations}\n  schema.sql: \
             {from_reference}"
        ));
    }
}

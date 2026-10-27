//! The URL of the database read from `[database] url_file` (spec 015, REQ-015-4; task
//! 015-3): the subcommands start without it on their command line, and it stays out of
//! their logs and errors.

use std::process::Stdio;
use std::time::Duration;

use sqlx::PgPool;

use crate::server;

/// The URL of the throwaway database of `pool`.
fn database_url(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").unwrap();
    let server = &base[..base.rfind('/').unwrap()];
    format!(
        "{server}/{}",
        pool.connect_options().get_database().unwrap()
    )
}

/// A directory of its own for `test`.
fn dir(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kartenn-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_server_tells_its_version() {
    let out = server().arg("--version").output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        format!("kartenn-server {}", env!("CARGO_PKG_VERSION"))
    );
}

#[kartenn_testing::pg_test]
async fn an_instance_starts_on_the_url_of_its_file(pool: PgPool) {
    let url = database_url(&pool);
    let dir = dir("url-file");
    let url_file = dir.join("database-url");
    std::fs::write(&url_file, format!("{url}\n")).unwrap();
    let log = dir.join("technical.jsonl");
    let config = dir.join("kartenn.toml");
    std::fs::write(
        &config,
        format!(
            "[telemetry]\noutputs = [{{ kind = \"file\", path = \"{}\", filter = \"debug\" }}]\n\
             [database]\nurl_file = \"{}\"\n",
            log.display(),
            url_file.display()
        ),
    )
    .unwrap();

    // The migration job reads the same file through --config.
    let migrated = server()
        .args(["migrate", "--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        migrated.status.success(),
        "{}",
        String::from_utf8_lossy(&migrated.stderr)
    );

    // Given twice, refused, without the URL in the error.
    let twice = server().arg("run").arg(&url).arg(&config).output().unwrap();
    assert!(!twice.status.success());
    let error = String::from_utf8_lossy(&twice.stderr);
    assert!(error.contains("twice"), "{error}");
    assert!(!error.contains(&url), "{error}");

    // An instance with no URL on its command line.
    let mut instance = server()
        .arg("run")
        .arg(&config)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut started = false;
    for _ in 0..300 {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit.event WHERE name = 'sys.instance.started'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if count > 0 {
            started = true;
            break;
        }
        assert!(
            instance.try_wait().unwrap().is_none(),
            "the instance stopped"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // The log file is written by a background thread: the audit event can land before it,
    // and a kill would then lose lines still buffered.
    for _ in 0..50 {
        if std::fs::metadata(&log).is_ok_and(|m| m.len() > 0) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    instance.kill().unwrap();
    let out = instance.wait_with_output().unwrap();
    assert!(started, "the instance starts on the URL of its file");
    let logs = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(!logs.is_empty(), "the instance logs");
    assert!(!logs.contains(&url), "the URL stays out of the logs");
    assert!(!String::from_utf8_lossy(&out.stderr).contains(&url));
}

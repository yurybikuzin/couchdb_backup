#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Error, Result};
#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn, Level};

use serde::{Deserialize, Serialize};

use common_macros::*;
declare_settings! {
    database: SettingsDatabase,
    task: SettingsTask,
    bucket: String, //“s3://data.example.com/folder”
    token: String, //“XXXXXXXXX”
    secret: String, //“YYYYYYYYYY”
    prefix: String, //“backup/ippbx”
    suffix: String, // “couchdb”
    loki: String, // “http://syslog-west.example.com:3100/loki/api/v1/push”
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDatabase {
    url: String,
    login: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsTask {
    weekly: SettingsTaskWeekly,
    monthly: SettingsTaskMonthly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsTaskWeekly {
    cron: String,
    databases: Vec<String>,
    delay: u64,
    chunk: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsTaskMonthly {
    cron: String,
    databases: Vec<String>,
    delay: u64,
    chunk: Option<u64>,
    backup_only_previus: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Weekly,
    Monthly,
}

pub async fn run(mode: Mode) -> Result<()> {
    let start = std::time::Instant::now();

    let regex_list = match mode {
        Mode::Weekly => settings!(task.weekly.databases).clone(),
        Mode::Monthly => settings!(task.monthly.databases).clone(),
    }
    .into_iter()
    .filter_map(|s| {
        regex::Regex::new(&s)
            .map_err(|err| eprintln!("failed Regex::new({s:?}: {err})"))
            .ok()
    })
    .collect::<Vec<_>>();
    if regex_list.is_empty() {
        bail!("regex_list.is_empty");
    }

    let (client, uri) = {
        let SettingsDatabase {
            url: uri,
            login: username,
            password,
        } = settings!(database).clone();
        (
            couch_rs::Client::new(&uri, &username, &password).map_err(|err| {
                anyhow!("failed establish connection to {uri:?} as user {username:?}: {err}")
            })?,
            uri,
        )
    };

    let mut db_list = client
        .list_dbs()
        .await
        .map_err(|err| anyhow!("failed list databases of {uri:?}: {err}"))?
        .into_iter()
        .filter(|s| regex_list.iter().any(|regex| regex.is_match(s)))
        .collect::<Vec<_>>();
    db_list.sort();
    println!(
        "selected {} database(s) for {mode:?} backup:\n{}",
        db_list.len(),
        db_list
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    for db_name in db_list {
        println!("will process db {db_name:?}");
        match client.db(&db_name).await {
            Err(err) => {
                eprintln!("failed to connect to db {db_name:?}: {err}");
            }
            Ok(db) => {
                let batch_size = match mode {
                    Mode::Weekly => settings!(task.weekly.chunk),
                    Mode::Monthly => settings!(task.monthly.chunk),
                }
                .unwrap_or_default(); // A value of 0, means the default batch_size of 1000 is used
                let max_results = 0; // max_results of 0 means all documents will be returned
                let (tx, mut rx) = tokio::sync::mpsc::channel::<
                    couch_rs::document::DocumentCollection<serde_json::value::Value>,
                >(1);

                tokio::spawn({
                    let db_name = db_name.clone();

                    let access_key = settings!(token).clone();
                    let secret_access_key = settings!(secret).clone();

                    let prefix = settings!(prefix).clone();
                    let suffix = settings!(suffix).clone();
                    let now = chrono::Utc::now();

                    use chrono::Datelike;
                    let day = now.day();
                    let month = now.month();
                    let year = now.year();

                    async move {
                        let mut chunk_id = 0;
                        while let Some(docs) = rx.recv().await {
                            match serde_json::to_string(&docs.rows) {
                                Err(err) => {
                                    eprintln!(
                                        "failed to serde_json::to_string db {db_name:?}: {err}"
                                    );
                                }
                                Ok(s) => {
                                    let mut e = flate2::write::GzEncoder::new(
                                        Vec::new(),
                                        flate2::Compression::default(),
                                    );
                                    use std::io::Write;

                                    match e.write_all(&s.into_bytes()) {
                                        Err(err) => {
                                            eprintln!(
                                                "failed to e.write_all() db {db_name:?}: {err}"
                                            );
                                        }
                                        Ok(()) => {
                                            match e.finish() {
                                                Err(err) => eprintln!(
                                                    "failed to e.finish() db {db_name:?}: {err}"
                                                ),
                                                Ok(compressed_bytes) => {
                                                    let bucket = settings!(bucket).clone();
                                                    match s3_bucket::S3BucketBuilder::new(bucket.clone())
                                                        .provider(s3_bucket::StaticProvider::new(
                                                            access_key.clone(),
                                                            secret_access_key.clone(),
                                                            None,
                                                            None,
                                                        ))
                                                        .build()
                                                        .map_err(|err| {
                                                            anyhow!(
                                                                "S3BucketBuilder::new({bucket:?}): {err}"
                                                            )
                                                        }) {
                                                        Err(err) => panic!("{err}"),
                                                        Ok(s3b) => {
                                                            let key = format!("{prefix}/{year}/{month:02}/{day:02}/{suffix}/{db_name}/{chunk_id:03}.json.gz");
                                                            let content_length = compressed_bytes.len() as i64;
                                                            let object_to_upload =
                                                                s3_bucket::ObjectToUploadBuilder::from_vecu8(
                                                                    compressed_bytes,
                                                                )
                                                                .content_length(Some(content_length))
                                                                .content_type(Some("application/json".to_owned()))
                                                                .build();
                                                            match s3b.upload(key.clone(), object_to_upload)
                                                                .await {
                                                                    Err(err) => {
                                                                        eprintln!("failed to upload {key:?} to {bucket:?}: {err}");
                                                                    }
                                                                    Ok(()) => {
                                                                        println!("did upload {key:?} to {bucket:?}");
                                                                    }
                                                                }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            chunk_id += 1;
                        }
                    }
                });

                match db.get_all_batched(tx, batch_size, max_results).await {
                    Err(err) => {
                        eprintln!("failed to connect to db {db_name:?}: {err}");
                    }
                    Ok(count) => {
                        println!("processed {count} docs from db {db_name:?}");
                    }
                }
            }
        }
    }

    println!(
        "OK: did complete {mode:?} backup in {}",
        arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis()),
    );

    Ok(())
}

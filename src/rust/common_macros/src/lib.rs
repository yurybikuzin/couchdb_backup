#[macro_export]
macro_rules! impl_from_ref {
    ($from:ty => $for:ty, $ident:ident, $($body:tt)*) => {
        impl From<&$from> for $for {
            fn from($ident: &$from) -> Self {
                $($body)*
            }
        }
        impl From<&mut $from> for $for {
            fn from($ident: &mut $from) -> Self {
                $($body)*
            }
        }
    };
    ($from:ty => $for:ty, $ident:ident : $type:ident, $($body:tt)*) => {
        impl From<&$from> for $for {
            fn from($ident: &$from) -> Self {
                $($body)*
            }
        }
        impl From<&mut $from> for $for {
            fn from($ident: &mut $from) -> Self {
                $($body)*
            }
        }
    };
    // for compatibility with impl_try_from
    ($from:ty, $for:ty, $ident:ident, $($body:tt)*) => {
        impl From<&$from> for $for {
            fn from($ident: &$from) -> Self {
                $($body)*
            }
        }
        impl From<&mut $from> for $for {
            fn from($ident: &mut $from) -> Self {
                $($body)*
            }
        }
    };
}

#[macro_export]
macro_rules! impl_from {
    ($from:ty => $for:ty, $ident:ident : $type:ident, $($body:tt)* ) => {
        impl From<$from> for $for {
            fn from($ident: $from) -> Self {
                type $type = $from;
                $($body)*
            }
        }
    };
    ($from:ty => $for:ty, $ident:ident, $($body:tt)* ) => {
        impl From<$from> for $for {
            fn from($ident: $from) -> Self {
                $($body)*
            }
        }
    };
    // for compatibility with impl_try_from
    ($from:ty => $for:ty, $error:ty, $ident:ident, $($body:tt)* ) => {
        impl From<$from> for $for {
            fn from($ident: $from) -> Self {
                let ret: Result<Self> = { $($body)* };
                ret.ok().unwrap()
            }
        }
    };
    // for compatibility with old code
    ($from:ty, $for:ty, $ident:ident, $($body:tt)* ) => {
        impl From<$from> for $for {
            fn from($ident: $from) -> Self {
                $($body)*
            }
        }
    };
}

#[macro_export]
macro_rules! impl_try_from {
    ($from:ty => $for:ty, $error:ty, $ident:ident, $($body:tt)* ) => {
        impl TryFrom<$from> for $for {
            type Error = $error;
            fn try_from($ident: $from) -> Result<Self, Self::Error> {
                $($body)*
            }
        }
    };
}

#[macro_export]
macro_rules! impl_display {
    ($for:ty, $self:ident, $f:ident, $($body:tt)+) => {
        impl std::fmt::Display for $for {
            fn fmt(&$self, $f: &mut std::fmt::Formatter) -> std::fmt::Result {
                $($body)+
            }
        }
    };
    ($for:ty, $self:ident, $fmt:literal, $($args:expr),+) => {
        impl std::fmt::Display for $for {
            fn fmt(&$self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    f,
                    $fmt,
                    $($args),+
                )
            }
        }
    }
}

pub mod declare_env_settings;
pub mod declare_settings;
pub mod pasitos;
pub mod will_did;

#[macro_export]
macro_rules! get_rwlock_opt {
    ($CACHE:expr, $do_what:expr, $($body:tt)*) => {{
        let val = get_rwlock_opt!(get => $CACHE);
        let val = if let Some(val) = val {
            val
        } else {
            will_did!(trace => $do_what, {
                let start = std::time::Instant::now();
                let val = {
                    $($body)*
                };
                get_rwlock_opt!(set => $CACHE, val);
                val
            })
        };
        val
    }};
    (get => $CACHE:expr) => {
        if let Some(val) = (*$CACHE.read().unwrap()).as_ref() {
            Some(val.clone())
        } else {
            None
        }
    };
    (set => $CACHE:expr, $val:expr) => {
        *$CACHE.write().unwrap() = Some($val.clone());
    };
}

#[macro_export]
macro_rules! pg {
    (pool => $POOLS:expr, $url:expr, $max_connections:expr, $for:expr) => {{
        let url: &str = $url.as_ref();
        let pool = if let Some(pool) = (*$POOLS.read().unwrap()).get(url) {
            Some(pool.clone())
        } else {
            None
        };
        let pool = if let Some(pool) = pool {
            pool
        } else {
            will_did!(trace => format!("get pool of {} for {}", url, $for), {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections($max_connections)
                    .connect(url)
                    .await?;
                (*$POOLS.write().unwrap()).insert(url.to_owned(), pool.clone());
                pool
            })
        };
        pool
    }};
}

// #[macro_export]
// macro_rules! mysql {
//     (pool => $POOLS:expr, $url:expr, $max_connections:expr, $for:expr) => {{
//         let url: &str = $url.as_ref();
//         let pool = if let Some(pool) = (*$POOLS.read().unwrap()).get(url) {
//             Some(pool.clone())
//         } else {
//             None
//         };
//         let pool = if let Some(pool) = pool {
//             pool
//         } else {
//             will_did!(trace => format!("get pool of {} for {}", url, $for), {
//                 let pool = sqlx::mysql::MySqlPoolOptions::new()
//                     .max_connections($max_connections)
//                     .connect(url)
//                     .await?;
//                 (*$POOLS.write().unwrap()).insert(url.to_owned(), pool.clone());
//                 pool
//             })
//         };
//         pool
//     }};
// }

#[macro_export]
macro_rules! r#impl {
    (FromStr for $type:ty; strum) => {
        impl std::str::FromStr for $type {
            type Err = anyhow::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use strum::IntoEnumIterator;
                if let Some(found) = Self::iter()
                    .find(|i| {
                        let eta = i.to_string();
                        let mut eta_iter = eta.chars();
                        let mut tst_iter = s.chars();
                        // let mut ret = true;
                        loop {
                            let eta = eta_iter.next();
                            let tst = tst_iter.next();
                            if eta.is_some() & tst.is_some() {
                                let eta = eta.unwrap();
                                let tst = tst.unwrap();
                                if !(eta == tst
                                    || match (eta, tst) {
                                        ('С', 'C') => true,
                                        ('с', 'c') => true,
                                        (_, _) => false,
                                    })
                                {
                                    break false;
                                }
                            } else if eta.is_none() && tst.is_none() {
                                break true;
                            } else {
                                break false;
                            }
                        }
                    })
                    .or_else(|| {
                        Self::iter().find(|i| {
                            let eta = i.to_string().to_lowercase();
                            let mut eta_iter = eta.chars();
                            let mut tst_iter = s.chars();
                            loop {
                                let eta = eta_iter.next();
                                let tst = tst_iter.next();
                                if eta.is_some() & tst.is_some() {
                                    let eta = eta.unwrap();
                                    let tst = tst.unwrap();
                                    if !(eta == tst
                                        || match (eta, tst) {
                                            ('с', 'c') => true,
                                            (_, _) => false,
                                        })
                                    {
                                        break false;
                                    }
                                } else if eta.is_none() && tst.is_none() {
                                    break true;
                                } else {
                                    break false;
                                }
                            }
                        })
                    })
                {
                    Ok(found)
                } else {
                    Err(anyhow!(
                        "failed {}::from_str({:?}): valid values: {}",
                        stringify!($type),
                        s,
                        Self::iter()
                            .map(|i| format!("{:?}", i.to_string()))
                            .collect::<Vec<String>>()
                            .join(", ")
                    ))
                }
            }
        }
    };
}

#[macro_export]
macro_rules! entry {
    ($hash_map:expr, $key:expr =>
         and_modify |$e:ident| $occupied:block
         or_insert $vacant:expr
    ) => {
        match $hash_map.entry($key) {
            std::collections::hash_map::Entry::Occupied(mut $e) => {
                let $e = $e.get_mut();
                $occupied;
            }
            std::collections::hash_map::Entry::Vacant($e) => {
                #[allow(unreachable_code)]
                $e.insert($vacant);
            }
        }
    };
    ($hash_map:expr, $key:expr =>
         and_modify_entry |$e:ident| $occupied:block
         or_insert_opt $vacant:expr
    ) => {
        match $hash_map.entry($key) {
            std::collections::hash_map::Entry::Occupied(mut $e) => {
                $occupied;
            }
            std::collections::hash_map::Entry::Vacant($e) => {
                if let Some(v) = $vacant {
                    $e.insert(v);
                }
            }
        }
    };
    ($hash_map:expr, $key:expr =>
         and_modify_entry |$e:ident| $occupied:block
         or_insert $vacant:expr
    ) => {
        match $hash_map.entry($key) {
            std::collections::hash_map::Entry::Occupied(mut $e) => {
                $occupied;
            }
            std::collections::hash_map::Entry::Vacant($e) => {
                #[allow(unreachable_code)]
                $e.insert($vacant);
            }
        }
    };
}

#[macro_export]
macro_rules! plural(
    ($count:expr, 1 $single:literal$(,)? 2 $some:literal$(,)? 5 $many:literal$(,)?) => {
        {
            let count = $count;
            (count % 100 / 10 != 1).then_some(0).and(
                match count % 10 {
                    1 => Some($single),
                    2 | 3 | 4 => Some($some),
                    _ => None,
                }
            ).unwrap_or($many)
        }
    };
    ($count:expr, 1 $single:expr, 2 $some:expr, 5 $many:expr$(,)?) => {
        {
            let count = $count;
            (count % 100 / 10 != 1).then_some(0).and(
                match count % 10 {
                    1 => Some($single),
                    2 | 3 | 4 => Some($some),
                    _ => None,
                }
            ).unwrap_or($many)
        }
    };
);

#[macro_export]
macro_rules! get_base_dir_and_args(
    ($args:ident, $config_path:expr) => {{
        fn get_base_dir_and_args() -> Result<Option<(Option<std::path::PathBuf>, Args)>> {
            let initial_dir = std::env::current_dir().ok();
            let $args = Args::parse();
            if !$args.no_show_opts {
                println!(
                    "{} {}",
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION"),
                );
            }
            if let Some(workdir) = $args.workdir.as_ref() {
                std::env::set_current_dir(workdir)
                    .map_err(|err| anyhow!("failed to set {workdir:?} for current dir: {err}"))?;
            }
            let mut did_change_current_dir_to_possible_working_folder = false;
            let env_file_name = std::path::PathBuf::from(".env");
            if env_file_name.exists() {
                dotenv::dotenv().map_err(|err| {
                    anyhow!(
                        "failed to open file '.env' in current dir {:?}: {err}",
                        std::env::current_dir().unwrap_or("unknown".into())
                    )
                })?;
            } else if $args.workdir.is_none() {
                let possible_working_folder = std::path::PathBuf::from(env!("CARGO_PKG_NAME"));
                let file_path = possible_working_folder.join(&env_file_name);
                if file_path.exists() {
                    dotenv::from_filename(&file_path).map_err(|err| {
                        anyhow!(
                            "failed to open file '{file_path:?}' in current dir {:?}: {err}",
                            std::env::current_dir().unwrap_or("unknown".into())
                        )
                    })?;
                    std::env::set_current_dir(&possible_working_folder).map_err(|err| {
                        anyhow!("failed to set {possible_working_folder:?} for current dir: {err}",)
                    })?;
                    println!(
                        "workdir changed to {possible_working_folder:?} for {env_file_name:?} found there"
                    );
                } else {
                    warn!(
                        "neighter '.env', nor '{file_path:?}' was found in current dir {:?}",
                        std::env::current_dir().unwrap_or("unknown".into())
                    );
                }
            }
            // pretty_env_logger::init_timed();
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish();
            tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
            if !$args.no_show_opts {
                println!(
                    "current dir: {:?}\nenv_settings: {:#?}",
                    std::env::current_dir().unwrap_or("unknown".into()),
                    *(ENV_SETTINGS.read().unwrap())
                );
            }
            let config_path: std::path::PathBuf = $config_path;
            let config_path = if config_path.is_absolute() || $args.workdir.is_some() || did_change_current_dir_to_possible_working_folder {
                config_path
            } else if config_path.exists() {
                config_path
            } else {
                let possible_working_folder = std::path::PathBuf::from(env!("CARGO_PKG_NAME"));
                let file_path = possible_working_folder.join(&config_path);
                if file_path.exists() {
                    std::env::set_current_dir(&possible_working_folder).map_err(|err| {
                        anyhow!("failed to set {possible_working_folder:?} for current dir: {err}",)
                    })?;
                    println!(
                        "workdir changed to {possible_working_folder:?} for {config_path:?} found there"
                    );
                }
                config_path
            };
            load_settings(&config_path).map_err(|err|
                if config_path.is_absolute() {
                    anyhow!(err)
                } else {
                    anyhow!("{err} while current_dir is {:?}", std::env::current_dir())
                }
            )?;
            if !$args.no_show_opts {
                println!(
                    "config is loaded from {:?}",
                    if config_path.is_absolute() {
                        config_path
                    } else if let Ok(current_dir) = std::env::current_dir() {
                        current_dir.join(config_path)
                    } else {
                        config_path
                    },
                );
                println!("args: {:#?}", $args);
            }
            Ok((!$args.test_config).then_some((
                if $args.workdir.is_some() {
                    None
                } else {
                    initial_dir
                },
                $args,
            )))
        }
        get_base_dir_and_args()
    }};
);

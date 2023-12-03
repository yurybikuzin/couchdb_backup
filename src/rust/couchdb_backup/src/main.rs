#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Error, Result};
#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn, Level};
// use aws_lambda_events::event::cloudwatch_events::CloudWatchEvent;use lambda_runtime::{run, service_fn, Error, LambdaEvent};

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
/// - https://github.com/aws-samples/serverless-rust-demo/
// async fn function_handler(event: LambdaEvent<CloudWatchEvent>) -> Result<(), Error> {
//     // Extract some useful information from the request
//
//     Ok(())
// }

// fn init_tracer() {
//     let subscriber = tracing_subscriber::fmt()
//         .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
//         .finish();
//     tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
// }

// #[tokio::main]
// async fn main() -> Result<()> {
//     init_tracer();
//     info!("HI");
//     Ok(())
//     // tracing_subscriber::fmt()
//     //
//     //     .with_max_level(tracing::Level::INFO)
//     //     // disable printing the name of the module in every log line.
//     //     .with_target(false)
//     //     // disabling time is handy because CloudWatch will add the ingestion time.
//     //     .without_time()
//     //     .init();
//
//     // run(service_fn(function_handler)).await
// }
use couchdb_backup::*;

use clap::Parser;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Workdir where to read .env and config, relative to current dir
    #[arg(short, long)]
    pub workdir: Option<std::path::PathBuf>,

    /// Config, relative to current dir (or workdir if it is set)
    #[arg(short, long)]
    pub config: Option<std::path::PathBuf>,

    /// Test config
    #[arg(short, long)]
    pub test_config: bool,

    /// No show opts
    #[arg(short, long)]
    pub no_show_opts: bool,

    #[command(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Weekly {},
    Monthly {},
}

use common_macros::*;
declare_env_settings! {
    config_path Option: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Some((_base_dir, args)) = get_base_dir_and_args!(args, {
        if let Some(config) = args.config.clone() {
            config
        } else if let Some(config_path) = env_settings!(config_path).clone() {
            config_path
        } else {
            std::path::PathBuf::from("config.yaml")
        }
    })? {
        match args.cmd {
            None => {}
            Some(Command::Weekly {}) => {
                couchdb_backup::run(Mode::Weekly).await?;
            }
            Some(Command::Monthly {}) => {
                couchdb_backup::run(Mode::Monthly).await?;
            }
        }
    }

    Ok(())
}

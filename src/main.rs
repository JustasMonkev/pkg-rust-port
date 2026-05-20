//! Command-line entrypoint for the Rust port of `pkg`.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(error) = run().await {
        // DECISION: the JS CLI writes fatal packaging errors to stdout with this
        // marker, and the oracle tests assert that channel and wording.
        println!("> Error! {error}");
        std::process::exit(2);
    }

    Ok(())
}

async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if let Some(chdir) = std::env::var_os("CHDIR") {
        let current_dir = std::env::current_dir()?;
        if chdir != current_dir.as_os_str() {
            std::env::set_current_dir(chdir)?;
        }
    }

    pkg_rust::exec(std::env::args_os().skip(1)).await?;
    Ok(())
}

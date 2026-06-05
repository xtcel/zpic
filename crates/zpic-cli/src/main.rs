use zpic_cli::cli::{run, Cli};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse_args();
    match run(cli).await {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(code) => std::process::exit(code),
    }
}

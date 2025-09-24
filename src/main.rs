use clap::Parser;
use dotenv::dotenv;
use luminis::run_with_config_path;

/// Luminis - система мониторинга и публикации новостей законодательства
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Путь к файлу конфигурации
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from `.env` file into std::env (optional)
    dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    // Load config, init logging and run
    run_with_config_path(&args.config).await
}

use dotenv::dotenv;
use luminis::run_with_config_path;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from `.env` file into std::env (optional)
    dotenv().ok();

    // Load config, init logging and run
    run_with_config_path("config.yaml").await
}

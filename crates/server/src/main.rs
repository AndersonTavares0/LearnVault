use learnvault_server::{Command, VERSION, parse_command, serve};

#[tokio::main]
async fn main() {
    match parse_command(std::env::args()) {
        Ok(Command::Version) => println!("learnvault-server {VERSION}"),
        Ok(Command::Run(config)) => {
            if let Err(error) = serve(config, shutdown_signal()).await {
                eprintln!("server startup failed: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("configuration error: {error}");
            std::process::exit(2);
        }
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

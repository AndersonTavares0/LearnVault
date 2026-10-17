use learnvault_server::{Command, VERSION, import_go_database, parse_command, serve};

fn main() {
    match parse_command(std::env::args()) {
        Ok(Command::Version) => println!("learnvault-server {VERSION}"),
        Ok(Command::MigrateGo(options)) => match import_go_database(options) {
            Ok(report) => println!(
                "users={} sessions={} media={} study_states={} removals={}",
                report.users, report.sessions, report.media, report.study_states, report.removals
            ),
            Err(error) => {
                eprintln!("migration failed: {error}");
                std::process::exit(1);
            }
        },
        Ok(Command::Run(config)) => run_server(config),
        Err(error) => {
            eprintln!("configuration error: {error}");
            std::process::exit(2);
        }
    }
}

#[tokio::main]
async fn run_server(config: learnvault_server::Config) {
    if let Err(error) = serve(config, shutdown_signal()).await {
        eprintln!("server startup failed: {error}");
        std::process::exit(1);
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

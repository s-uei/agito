use agito::{ssh, web};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(name = "agito-server")]
#[command(about = "Agito Git Server", long_about = None)]
struct Args {
    /// Directory to store repositories
    #[arg(long, default_value = "/var/lib/agito/repos")]
    repos: PathBuf,

    /// HTTP port for web viewer
    #[arg(long, default_value = "3000")]
    http_port: String,

    /// SSH port for git operations
    #[arg(long, default_value = "2222")]
    ssh_port: String,

    /// SSH host key file
    #[arg(long, default_value = "/var/lib/agito/ssh/host_key")]
    ssh_key: PathBuf,

    /// Authorized keys file
    #[arg(long, default_value = "/var/lib/agito/ssh/authorized_keys")]
    authorized_keys: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Create directories if they don't exist
    std::fs::create_dir_all(&args.repos)?;
    
    if let Some(parent) = args.ssh_key.parent() {
        std::fs::create_dir_all(parent)?;
    }

    tracing::info!("Agito Server Starting...");
    tracing::info!("Repositories: {:?}", args.repos);
    tracing::info!("HTTP Port: {}", args.http_port);
    tracing::info!("SSH Port: {}", args.ssh_port);

    // Start SSH server in a task
    let ssh_server = ssh::Server::new(
        args.ssh_port.clone(),
        args.ssh_key,
        args.authorized_keys,
        args.repos.clone(),
    );
    
    let ssh_handle = tokio::spawn(async move {
        if let Err(e) = ssh_server.start().await {
            tracing::error!("SSH server error: {}", e);
        }
    });

    // Start HTTP server in a task
    let web_server = web::WebServer::new(args.repos);
    let http_port = args.http_port.clone();
    
    let web_handle = tokio::spawn(async move {
        if let Err(e) = web_server.start(&http_port).await {
            tracing::error!("Web server error: {}", e);
        }
    });

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Shutdown signal received");
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    tracing::info!("Shutting down...");
    
    // In a production system, we'd gracefully shutdown servers here
    ssh_handle.abort();
    web_handle.abort();

    Ok(())
}

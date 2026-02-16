//! HAL Enforcement Service - Main entry point

use clap::Parser;
use colored::Colorize;
use hal_enforcement_service::{EnforcementService, api};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "hal-enforcer")]
#[command(about = "Policy-based enforcement service for ELASTIC TEE HAL", long_about = None)]
struct Cli {
    /// Path to policy YAML file
    #[arg(short, long, value_name = "FILE")]
    policy: PathBuf,
    
    /// Port to listen on
    #[arg(short = 'P', long, default_value = "8080")]
    port: u16,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Validate policy only (don't start service)
    #[arg(long)]
    validate_only: bool,
    
    /// List entities in policy
    #[arg(long)]
    list_entities: bool,
    
    /// Audit log file path (overrides policy setting)
    #[arg(long)]
    audit_log: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Setup logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();
    
    // Print banner
    print_banner();
    
    // Load and validate policy
    println!("{}", "Loading policy configuration...".cyan());
    let service = EnforcementService::from_file(&cli.policy)?;
    println!("{} Policy loaded successfully", "✓".green());
    
    if cli.validate_only {
        println!("{}", "Policy validation successful!".green().bold());
        return Ok(());
    }
    
    if cli.list_entities {
        println!("\n{}", "Entities in policy:".cyan().bold());
        for (i, entity) in service.list_entities().iter().enumerate() {
            if let Some(config) = service.get_entity_config(entity) {
                println!("  {}. {} - {}", 
                    (i + 1).to_string().yellow(),
                    entity.green(),
                    config.description
                );
                println!("     Capabilities: {}", 
                    config.capabilities.list_granted().join(", ").cyan()
                );
            }
        }
        println!();
        return Ok(());
    }
    
    // Start HTTP API server
    println!("\n{}", "Starting HTTP API server...".cyan());
    let service = Arc::new(service);
    let app = api::create_router(service.clone());
    
    let addr = format!("0.0.0.0:{}", cli.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    println!("{} Server listening on {}", 
        "✓".green(),
        format!("http://{}", addr).cyan().bold()
    );
    println!("\n{}", "Endpoints:".cyan().bold());
    println!("  {} http://{}/health", "GET".yellow(), addr);
    println!("  {} http://{}/api/v1/hal/access", "POST".yellow(), addr);
    println!("  {} http://{}/api/v1/entities", "GET".yellow(), addr);
    println!("  {} http://{}/api/v1/audit", "GET".yellow(), addr);
    
    println!("\n{}", "Service ready!".green().bold());
    println!("Press Ctrl+C to stop\n");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

fn print_banner() {
    println!("\n{}", "═══════════════════════════════════════════════════".cyan());
    println!("{}", "  HAL ENFORCEMENT SERVICE".cyan().bold());
    println!("{}", "  Policy-based access control for WASMHAL".cyan());
    println!("{}", format!("  Version {}", hal_enforcement_service::VERSION).cyan());
    println!("{}", "═══════════════════════════════════════════════════".cyan());
    println!();
}

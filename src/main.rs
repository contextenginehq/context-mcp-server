use mcp_context_server::config::ServerConfig;
use mcp_context_server::server::McpServer;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = match ServerConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mcp-context-server: configuration error: {e}");
            std::process::exit(1);
        }
    };

    let mut server = McpServer::new(config);
    if let Err(e) = server.run().await {
        eprintln!("mcp-context-server: fatal error: {e}");
        std::process::exit(1);
    }
}

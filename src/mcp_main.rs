fn main() {
    env_logger::init();

    if let Err(e) = falcon::mcp::server::run_mcp_server() {
        eprintln!("Falcon MCP server error: {}", e);
        std::process::exit(1);
    }
}

fn main() {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("falcon-mcp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    env_logger::init();

    if let Err(e) = falcon::mcp::server::run_mcp_server() {
        eprintln!("Falcon MCP server error: {}", e);
        std::process::exit(1);
    }
}

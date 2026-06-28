use falcon::lsp::server::FalconLspServer;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("falcon-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(FalconLspServer::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}

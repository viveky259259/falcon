use falcon::lsp::server::FalconLspServer;
use tower_lsp::{LspService, Server};

fn main() {
    // Windows' 1 MB main-thread stack is too small for analysis, so the
    // server runs on a thread with a larger stack, as do the runtime's workers.
    falcon::big_stack::run(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(falcon::big_stack::STACK_BYTES)
            .build();
        match runtime {
            Ok(runtime) => runtime.block_on(lsp_main()),
            Err(e) => {
                eprintln!("falcon-lsp: could not start the async runtime: {e}");
                std::process::exit(1);
            }
        }
    });
}

async fn lsp_main() {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("falcon-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(FalconLspServer::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}

mod backend;
mod convert;
mod embedded_protocol;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(backend::RainbowLanguageServer::new)
        .custom_method(
            "rainbow/embeddedRegions",
            backend::RainbowLanguageServer::embedded_regions,
        )
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

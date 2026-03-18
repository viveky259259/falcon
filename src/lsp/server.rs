use crate::config::FalconConfig;
use crate::lsp::actions::generate_code_actions;
use crate::lsp::diagnostics::{analyze_source, issues_to_diagnostics};
use crate::rules::RuleRegistry;
use dashmap::DashMap;
use serde_json::Value;
use std::path::PathBuf;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub struct FalconLspServer {
    pub client: Client,
    pub config: tokio::sync::RwLock<FalconConfig>,
    pub documents: DashMap<Url, String>,
    pub diagnostics_cache: DashMap<Url, Vec<Diagnostic>>,
}

impl FalconLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            config: tokio::sync::RwLock::new(FalconConfig::default()),
            documents: DashMap::new(),
            diagnostics_cache: DashMap::new(),
        }
    }

    async fn analyze_and_publish(&self, uri: &Url, source: &str) {
        let file_path = uri_to_path(uri);
        let config = self.config.read().await.clone();

        let mut registry = RuleRegistry::new();
        registry.register_defaults(&config);

        let issues = analyze_source(source, &file_path, &config, &registry);
        let diagnostics = issues_to_diagnostics(&issues);

        self.diagnostics_cache
            .insert(uri.clone(), diagnostics.clone());

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn try_load_config(&self, workspace_path: &std::path::Path) {
        if let Ok(config) = FalconConfig::load(workspace_path) {
            let mut cfg = self.config.write().await;
            *cfg = config;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for FalconLspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Some(path) = uri_to_path_opt(&root_uri) {
                self.try_load_config(&path).await;
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                        resolve_provider: Some(false),
                        ..Default::default()
                    },
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "falcon.fixAll".to_string(),
                        "falcon.analyze".to_string(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Falcon".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "Falcon LSP server v{} initialized",
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let source = params.text_document.text;

        if !uri.as_str().ends_with(".dart") {
            return;
        }

        self.documents.insert(uri.clone(), source.clone());
        self.analyze_and_publish(&uri, &source).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        if !uri.as_str().ends_with(".dart") {
            return;
        }

        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents.insert(uri.clone(), change.text.clone());
            self.analyze_and_publish(&uri, &change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        if !uri.as_str().ends_with(".dart") {
            return;
        }

        if let Some(source) = params.text {
            self.documents.insert(uri.clone(), source.clone());
            self.analyze_and_publish(&uri, &source).await;
        } else if let Some(source) = self.documents.get(&uri) {
            let source = source.clone();
            self.analyze_and_publish(&uri, &source).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        self.diagnostics_cache.remove(&uri);
        self.client
            .publish_diagnostics(uri, Vec::new(), None)
            .await;
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;

        let source = match self.documents.get(uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };

        let diagnostics_in_range: Vec<Diagnostic> = params
            .context
            .diagnostics
            .iter()
            .filter(|d| d.source.as_deref() == Some("falcon"))
            .cloned()
            .collect();

        if diagnostics_in_range.is_empty() {
            return Ok(None);
        }

        let actions = generate_code_actions(uri, &diagnostics_in_range, &source);

        Ok(Some(actions))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            "falcon.analyze" => {
                for entry in self.documents.iter() {
                    let uri = entry.key().clone();
                    let source = entry.value().clone();
                    self.analyze_and_publish(&uri, &source).await;
                }
                self.client
                    .log_message(MessageType::INFO, "Falcon: Re-analyzed all open files")
                    .await;
            }
            "falcon.fixAll" => {
                self.client
                    .log_message(MessageType::INFO, "Falcon: Fix all command executed")
                    .await;
            }
            _ => {}
        }
        Ok(None)
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        if let Some(settings) = params.settings.as_object() {
            if let Some(falcon_settings) = settings.get("falcon") {
                let auto_fix_on_save = falcon_settings
                    .get("autoFixOnSave")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Falcon: autoFixOnSave = {}", auto_fix_on_save),
                    )
                    .await;
            }
        }

        for entry in self.documents.iter() {
            let uri = entry.key().clone();
            let source = entry.value().clone();
            self.analyze_and_publish(&uri, &source).await;
        }
    }
}

fn uri_to_path(uri: &Url) -> PathBuf {
    uri.to_file_path().unwrap_or_else(|_| {
        PathBuf::from(uri.path())
    })
}

fn uri_to_path_opt(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

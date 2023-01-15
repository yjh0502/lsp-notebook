use log::*;
use serde_json::*;
use std::collections::HashMap;
use std::sync::*;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::*;

#[derive(Clone, Debug)]
struct FileState {
    uri: Url,
    tree: Tree,
    #[allow(unused)]
    content: String,
}

#[derive(Debug)]
struct Backend {
    #[allow(unused)]
    client: Client,

    tree: Arc<Mutex<HashMap<Url, FileState>>>,
}

impl Backend {
    fn update(&self, uri: Url, content: &str) {
        let mut guard = self.tree.lock().unwrap();
        let tree = lsp_notebook::parse(content);

        guard.insert(
            uri.clone(),
            FileState {
                uri,
                content: content.to_owned(),
                tree,
            },
        );
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(true),
                }),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["lsp-notebook.run".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.update(params.text_document.uri.clone(), &params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let changes = params.content_changes;
        self.update(params.text_document.uri.clone(), changes[0].text.as_str());
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        info!("did_change_configuration: {:?}", params);
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let lens = {
            let guard = self.tree.lock().unwrap();
            let nodes = match guard.get(&params.text_document.uri) {
                Some(state) => lsp_notebook::code_actions(&state.tree, &state.content),
                None => vec![],
            };
            nodes
                .into_iter()
                .map(|(node, output)| {
                    let mut args = vec![json!(params.text_document.uri.clone()), json!(node.id())];
                    if let Some(output) = output {
                        args.push(json!(output.id()));
                    }

                    lsp_types::CodeLens {
                        range: lsp_notebook::node_range(node),
                        command: Some(lsp_types::Command {
                            title: "Run".to_string(),
                            command: "lsp-notebook.run".to_string(),
                            arguments: Some(args),
                        }),
                        data: None,
                    }
                })
                .collect()
        };

        info!("lens={:?}", lens);

        Ok(Some(lens))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        info!("execute_command: {:?}", params);

        let uri = Url::parse(params.arguments[0].as_str().unwrap()).unwrap();
        let node_id = params.arguments[1].as_u64().unwrap() as usize;
        let output_id = params
            .arguments
            .get(2)
            .map(|v| v.as_u64().unwrap() as usize);

        let mut changes = HashMap::new();
        {
            let state = match self.tree.lock().unwrap().get(&uri) {
                Some(state) => state.clone(),
                None => return Ok(None),
            };

            let node = lsp_notebook::node_by_id(state.tree.root_node(), node_id).unwrap();
            let code_content = lsp_notebook::code_content(node, &state.content);

            use std::io::Write;
            use std::process::{Command, Stdio};

            let mut child = Command::new("/bin/sh")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("failed to execute child");

            let mut stdin = child.stdin.take().expect("failed to get stdin");
            std::thread::spawn(move || {
                stdin
                    .write_all(code_content.as_bytes())
                    .expect("failed to write to stdin");
            });

            let output = child.wait_with_output().expect("failed to wait on child");
            let stdout = std::str::from_utf8(&output.stdout).unwrap();

            let range = match output_id {
                Some(output_id) => {
                    let output_node =
                        lsp_notebook::node_by_id(state.tree.root_node(), output_id).unwrap();
                    lsp_notebook::node_range(output_node)
                }
                None => lsp_types::Range {
                    start: lsp_notebook::pos_ts_to_lsp(node.end_position()),
                    end: lsp_notebook::pos_ts_to_lsp(node.end_position()),
                },
            };
            let newline = if output_id.is_some() { "" } else { "\n" };
            info!("range={:?}", range);
            changes.insert(
                state.uri.clone(),
                vec![TextEdit {
                    new_text: format!("{}```output {}\n{}\n```", newline, output.status, stdout),
                    range,
                }],
            );
        };

        self.client
            .apply_edit(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            })
            .await
            .unwrap();

        Ok(None)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    use simplelog::*;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            std::fs::File::create("lsp.log").expect("std::fs::File::create"),
        ),
    ])
    .unwrap();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        tree: Default::default(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

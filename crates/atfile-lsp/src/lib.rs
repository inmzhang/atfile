use std::{collections::HashMap, path::Path};

use globset::{Glob, GlobSet, GlobSetBuilder};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, Documentation, InitializeParams,
    InsertTextFormat, Position, Range, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri,
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
    request::{Completion, Request as LspRequest},
};
use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{CaseMatching, Normalization, Pattern},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct AtfileConfig {
    enabled_languages: Vec<String>,
    enabled_path_suffixes: Vec<String>,
    include_hidden: bool,
    ignored_globs: Vec<String>,
    max_results: usize,
    insert_prefix: String,
}

impl Default for AtfileConfig {
    fn default() -> Self {
        Self {
            enabled_languages: vec![
                "markdown".to_string(),
                "plaintext".to_string(),
                "git-commit".to_string(),
                "yaml".to_string(),
                "json".to_string(),
                "toml".to_string(),
            ],
            enabled_path_suffixes: Vec::new(),
            include_hidden: false,
            ignored_globs: vec![
                ".git/**".to_string(),
                "node_modules/**".to_string(),
                "target/**".to_string(),
                "dist/**".to_string(),
                "build/**".to_string(),
            ],
            max_results: 200,
            insert_prefix: "@".to_string(),
        }
    }
}

impl AtfileConfig {
    fn ignored_glob_set(&self) -> Result<GlobSet, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &self.ignored_globs {
            builder.add(Glob::new(pattern)?);
        }
        builder.build()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathCandidate {
    path: String,
    is_dir: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ReferenceContext {
    start: usize,
    end: usize,
    query: String,
}

pub fn run_stdio() -> anyhow::Result<()> {
    let (connection, io_threads) = Connection::stdio();
    let initialize_params = connection.initialize(serde_json::to_value(server_capabilities())?)?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params)?;
    let mut state = ServerState::new(initialize_params);
    state.event_loop(&connection)?;
    io_threads.join()?;
    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["@".to_string()]),
            ..Default::default()
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..Default::default()
    }
}

struct ServerState {
    documents: HashMap<String, DocumentState>,
    config: AtfileConfig,
    candidates: Vec<PathCandidate>,
}

struct DocumentState {
    text: String,
    language_id: String,
}

impl ServerState {
    fn new(params: InitializeParams) -> Self {
        let root = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|folder| file_uri_to_path(&folder.uri));
        let config = params
            .initialization_options
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        let candidates = root
            .as_ref()
            .and_then(|root| scan_workspace(root, &config).ok())
            .unwrap_or_default();

        Self {
            documents: HashMap::new(),
            config,
            candidates,
        }
    }

    fn event_loop(&mut self, connection: &Connection) -> anyhow::Result<()> {
        for message in &connection.receiver {
            match message {
                Message::Request(request) => {
                    if connection.handle_shutdown(&request)? {
                        break;
                    }
                    self.handle_request(connection, request)?;
                }
                Message::Response(_) => {}
                Message::Notification(notification) => {
                    if notification.method == DidOpenTextDocument::METHOD {
                        let params: DidOpenTextDocumentParams =
                            serde_json::from_value(notification.params)?;
                        self.documents.insert(
                            params.text_document.uri.to_string(),
                            DocumentState {
                                text: params.text_document.text,
                                language_id: params.text_document.language_id,
                            },
                        );
                    } else if notification.method == DidChangeTextDocument::METHOD {
                        let params: DidChangeTextDocumentParams =
                            serde_json::from_value(notification.params)?;
                        if let Some(change) = params.content_changes.into_iter().last() {
                            self.apply_full_change(params.text_document.uri.to_string(), change);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_request(&mut self, connection: &Connection, request: Request) -> anyhow::Result<()> {
        match request.method.as_str() {
            Completion::METHOD => {
                let id = request.id;
                let params: CompletionParams = serde_json::from_value(request.params)?;
                connection.sender.send(Message::Response(Response {
                    id,
                    result: Some(serde_json::to_value(self.completion_response(params))?),
                    error: None,
                }))?;
            }
            _ => {
                connection.sender.send(Message::Response(Response {
                    id: request.id,
                    result: None,
                    error: Some(lsp_server::ResponseError {
                        code: lsp_server::ErrorCode::MethodNotFound as i32,
                        message: "method not found".to_string(),
                        data: None,
                    }),
                }))?;
            }
        }
        Ok(())
    }

    fn apply_full_change(&mut self, uri: String, change: TextDocumentContentChangeEvent) {
        if change.range.is_none()
            && let Some(document) = self.documents.get_mut(&uri)
        {
            document.text = change.text;
        }
    }

    fn completion_response(&self, params: CompletionParams) -> Option<CompletionResponse> {
        let text_document_position = params.text_document_position;
        let uri = text_document_position.text_document.uri.to_string();
        let document = self.documents.get(&uri)?;
        if !self.language_enabled(&document.language_id) {
            return None;
        }
        let position = text_document_position.position;
        let line = document.text.lines().nth(position.line as usize)?;
        let context = reference_context_at(line, position.character as usize)?;
        Some(CompletionResponse::Array(completion_items(
            &context,
            &self.candidates,
            &self.config,
            position.line,
        )))
    }

    fn language_enabled(&self, language_id: &str) -> bool {
        self.config.enabled_languages.is_empty()
            || self
                .config
                .enabled_languages
                .iter()
                .any(|enabled| enabled.eq_ignore_ascii_case(language_id))
    }
}

fn reference_context_at(line: &str, cursor: usize) -> Option<ReferenceContext> {
    if cursor > line.len() || !line.is_char_boundary(cursor) {
        return None;
    }

    let before_cursor = &line[..cursor];
    let mut at_index = None;
    for (index, ch) in before_cursor.char_indices().rev() {
        if ch == '@' {
            at_index = Some(index);
            break;
        }
        if !is_reference_char(ch) {
            break;
        }
    }

    let start = at_index?;
    if start > 0 && is_reference_char(line[..start].chars().next_back()?) {
        return None;
    }

    let query = &line[start + 1..cursor];
    if !query.chars().all(is_reference_char) {
        return None;
    }

    Some(ReferenceContext {
        start,
        end: cursor,
        query: query.to_string(),
    })
}

fn is_reference_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-')
}

fn scan_workspace(root: &Path, config: &AtfileConfig) -> anyhow::Result<Vec<PathCandidate>> {
    let ignored_globs = config.ignored_glob_set()?;
    let mut candidates = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(!config.include_hidden)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        if path == root {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if ignored_globs.is_match(relative)
            || ignored_globs.is_match(format!("{}/", relative.display()))
        {
            continue;
        }

        let mut relative = relative.to_string_lossy().replace('\\', "/");
        if relative.is_empty() {
            continue;
        }
        let is_dir = entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir());
        if is_dir && !relative.ends_with('/') {
            relative.push('/');
        }
        candidates.push(PathCandidate {
            path: relative,
            is_dir,
        });
    }

    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(candidates)
}

fn completion_items(
    context: &ReferenceContext,
    candidates: &[PathCandidate],
    config: &AtfileConfig,
    line: u32,
) -> Vec<CompletionItem> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&context.query, CaseMatching::Smart, Normalization::Smart);
    let mut matches = candidates
        .iter()
        .filter(|candidate| path_suffix_enabled(candidate, config))
        .filter_map(|candidate| {
            let path = Utf32String::from(candidate.path.as_str());
            pattern
                .score(path.slice(..), &mut matcher)
                .map(|score| (score, candidate))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });

    matches
        .into_iter()
        .take(config.max_results)
        .map(|(_, candidate)| {
            let range = Range {
                start: Position {
                    line,
                    character: context.start as u32,
                },
                end: Position {
                    line,
                    character: context.end as u32,
                },
            };
            let insert_text = format!("{}{}", config.insert_prefix, candidate.path);
            CompletionItem {
                label: insert_text.clone(),
                kind: Some(if candidate.is_dir {
                    CompletionItemKind::FOLDER
                } else {
                    CompletionItemKind::FILE
                }),
                filter_text: Some(candidate.path.clone()),
                sort_text: Some(candidate.path.clone()),
                detail: Some(if candidate.is_dir {
                    "directory".to_string()
                } else {
                    "file".to_string()
                }),
                documentation: Some(Documentation::String(candidate.path.clone())),
                text_edit: Some(TextEdit::new(range, insert_text).into()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            }
        })
        .collect()
}

fn path_suffix_enabled(candidate: &PathCandidate, config: &AtfileConfig) -> bool {
    candidate.is_dir
        || config.enabled_path_suffixes.is_empty()
        || config
            .enabled_path_suffixes
            .iter()
            .any(|suffix| candidate.path.ends_with(suffix))
}

fn file_uri_to_path(uri: &Uri) -> Option<std::path::PathBuf> {
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lsp_types::CompletionTextEdit;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn detects_at_reference_and_ignores_email() {
        assert_eq!(
            reference_context_at("see @src/lib", 12),
            Some(ReferenceContext {
                start: 4,
                end: 12,
                query: "src/lib".to_string(),
            })
        );
        assert_eq!(reference_context_at("me@example.com", 3), None);
    }

    #[test]
    fn creates_ranked_file_completion_with_text_edit() {
        let context = ReferenceContext {
            start: 4,
            end: 8,
            query: "src".to_string(),
        };
        let items = completion_items(
            &context,
            &[PathCandidate {
                path: "src/lib.rs".to_string(),
                is_dir: false,
            }],
            &AtfileConfig::default(),
            2,
        );

        assert_eq!(items[0].label, "@src/lib.rs");
        assert_eq!(items[0].kind, Some(CompletionItemKind::FILE));
        match items[0].text_edit.as_ref().unwrap() {
            CompletionTextEdit::Edit(edit) => {
                assert_eq!(edit.range.start.line, 2);
                assert_eq!(edit.range.start.character, 4);
                assert_eq!(edit.new_text, "@src/lib.rs");
            }
            CompletionTextEdit::InsertAndReplace(_) => panic!("expected simple text edit"),
        }
    }

    #[test]
    fn scans_workspace_and_ignores_vendor_directories() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        fs::write(dir.path().join("node_modules/pkg/index.js"), "").unwrap();

        let candidates = scan_workspace(dir.path(), &AtfileConfig::default()).unwrap();

        assert!(candidates.contains(&PathCandidate {
            path: "README.md".to_string(),
            is_dir: false,
        }));
        assert!(candidates.contains(&PathCandidate {
            path: "src/".to_string(),
            is_dir: true,
        }));
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.path.starts_with("node_modules/"))
        );
    }
}

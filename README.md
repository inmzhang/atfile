# Atfile

Atfile is a Zed extension that provides file and directory path completions after `@` in normal files just like in a coding agent.

The extension attach a dedicated lsp server to the file for the completion task.

## Settings

Pass settings through the LSP initialization options for `atfile`:

```json
{
  "lsp": {
    "atfile": {
      "initialization_options": {
        "enabled_languages": ["markdown", "plaintext", "git-commit", "yaml", "json", "toml"],
        "enabled_path_suffixes": [],
        "include_hidden": false,
        "ignored_globs": [".git/**", "node_modules/**", "target/**", "dist/**", "build/**"],
        "max_results": 200,
        "insert_prefix": "@"
      }
    }
  }
}
```

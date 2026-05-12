use zed_extension_api as zed;

mod installation;

const SERVER_ID: &str = "atfile";

struct AtfileExtension;

impl zed::Extension for AtfileExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!("unknown language server: {language_server_id}"));
        }

        if let Some(command) = worktree.which(installation::SERVER_BINARY) {
            return Ok(zed::Command {
                command,
                args: vec!["--stdio".to_string()],
                env: Default::default(),
            });
        }

        let command = installation::installed_server_binary()?;

        Ok(zed::Command {
            command,
            args: vec!["--stdio".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AtfileExtension);

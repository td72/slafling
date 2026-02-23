mod cli;
mod config;
mod keychain;
mod slack;
mod token;

use std::io::{BufRead, IsTerminal, Read, Write};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let env = config::Env::load();

    let headless = cli.headless || env.headless;

    // Handle commands that don't need a fully resolved Config
    match &cli.command {
        Some(cli::Command::Init) => {
            if headless {
                bail!("init is not available in headless mode");
            }
            return run_init();
        }
        Some(cli::Command::Token { action }) => {
            if headless {
                bail!("token is not available in headless mode");
            }
            let profile = cli.profile.as_deref().or(env.profile.as_deref());
            return run_token(action, profile);
        }
        Some(cli::Command::Validate) => {
            if headless {
                bail!("validate has no effect in headless mode");
            }
            let path = config::config_path()?;
            config::load_config()?;
            println!("{}: ok", path.display());
            return Ok(());
        }
        _ => {}
    }

    let config = if headless {
        if cli.profile.is_some() || env.profile.is_some() {
            eprintln!("warning: --profile is ignored in headless mode");
        }
        config::Config::new(None, None, &env)?
    } else {
        let file = config::load_config()?;
        let profile = cli.profile.as_deref().or(env.profile.as_deref());
        config::Config::new(Some(&file), profile, &env)?
    };

    match cli.command {
        Some(cli::Command::Search {
            query,
            output,
            types,
        }) => run_search(&config, &query, output, types),
        None => run_send(&config, cli.send),
        _ => unreachable!(),
    }
}

fn run_init() -> Result<()> {
    let path = config::config_path()?;

    if path.exists() {
        if !std::io::stdin().is_terminal() {
            bail!(
                "{} already exists (run interactively to confirm overwrite)",
                path.display()
            );
        }
        if !confirm_yes_no(&format!(
            "{} already exists. Overwrite? [y/N] ",
            path.display()
        ))? {
            bail!("aborted");
        }
    }

    let token_value = prompt_token("init")?;

    // Store token using platform default (config doesn't exist yet)
    store_token(
        config::TokenStore::default_for_platform(),
        None,
        &token_value,
    )?;

    // Write config without token
    config::write_init_config(&path)?;

    println!("created {}", path.display());
    Ok(())
}

fn confirm_yes_no(prompt: &str) -> Result<bool> {
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut input = String::new();
    std::io::stdin().lock().read_line(&mut input)?;
    Ok(matches!(input.trim(), "y" | "Y"))
}

fn prompt_token(command: &str) -> Result<String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("{command} requires interactive input (stdin must be a TTY)");
    }
    eprint!("Bot Token (xoxb-...): ");
    std::io::stderr().flush()?;
    let mut buf = String::new();
    stdin.lock().read_line(&mut buf)?;
    let value = buf.trim().to_string();
    if value.is_empty() {
        bail!("token is required");
    }
    Ok(value)
}

fn store_token(
    token_store: config::TokenStore,
    profile: Option<&str>,
    token_value: &str,
) -> Result<()> {
    match token_store {
        config::TokenStore::Keychain => {
            keychain::set_token(profile, token_value)?;
            let account = profile.unwrap_or("default");
            eprintln!("token stored in Keychain (account: {account})");
        }
        config::TokenStore::File => {
            token::set_token(profile, token_value)?;
            let path = token::token_path(profile)?;
            eprintln!("token stored in {}", path.display());
        }
    }
    Ok(())
}

/// Load token_store from config file, falling back to platform default if config doesn't exist.
fn load_token_store() -> Result<config::TokenStore> {
    let path = config::config_path()?;
    if !path.exists() {
        return Ok(config::TokenStore::default_for_platform());
    }
    let cfg = config::load_config()?;
    Ok(config::resolve_token_store(&cfg))
}

fn run_token(action: &cli::TokenAction, profile: Option<&str>) -> Result<()> {
    match action {
        cli::TokenAction::Set => run_token_set(profile),
        cli::TokenAction::Delete => run_token_delete(profile),
        cli::TokenAction::Show => run_token_show(profile),
    }
}

fn run_token_set(profile: Option<&str>) -> Result<()> {
    let token_value = prompt_token("token set")?;
    let token_store = load_token_store()?;
    store_token(token_store, profile, &token_value)?;
    Ok(())
}

fn run_token_delete(profile: Option<&str>) -> Result<()> {
    let token_store = load_token_store()?;

    match token_store {
        config::TokenStore::Keychain => {
            let account = profile.unwrap_or("default");
            if keychain::get_token(profile)?.is_none() {
                bail!("no stored token found for profile '{account}'");
            }
            keychain::delete_token(profile)?;
            eprintln!("deleted token from Keychain (account: {account})");
        }
        config::TokenStore::File => {
            let path = token::token_path(profile)?;
            if !path.exists() {
                let name = profile.unwrap_or("default");
                bail!("no stored token found for profile '{name}'");
            }
            token::delete_token(profile)?;
            eprintln!("deleted {}", path.display());
        }
    }

    Ok(())
}

fn run_token_show(profile: Option<&str>) -> Result<()> {
    let token_store = load_token_store()?;
    match config::describe_token_source(token_store, profile) {
        Ok((source, location)) => {
            println!("source: {source}");
            println!("location: {location}");
        }
        Err(e) => {
            println!("not configured: {e}");
        }
    }
    Ok(())
}

fn run_search(
    config: &config::Config,
    query: &str,
    cli_output: Option<cli::OutputFormat>,
    types: Option<Vec<cli::SearchType>>,
) -> Result<()> {
    let token = config.resolve_token()?;
    let types = types.unwrap_or_else(|| {
        config
            .search_types
            .clone()
            .unwrap_or_else(|| vec![cli::SearchType::PublicChannel])
    });
    let format = resolve_output_format(cli_output, config.output);

    run_search_with_token(&token, query, format, &types)
}

fn run_search_with_token(
    token: &str,
    query: &str,
    format: cli::OutputFormat,
    types: &[cli::SearchType],
) -> Result<()> {
    let channels = slack::search_channels(token, query, types)?;

    if channels.is_empty() {
        eprintln!("no channels matching '{query}'");
        std::process::exit(1);
    }

    match format {
        cli::OutputFormat::Table => print_table(&channels),
        cli::OutputFormat::Tsv => print_tsv(&channels),
        cli::OutputFormat::Json => print_json(&channels)?,
    }

    Ok(())
}

fn resolve_output_format(
    cli_output: Option<cli::OutputFormat>,
    config_output: Option<cli::OutputFormat>,
) -> cli::OutputFormat {
    // 1. CLI flag
    if let Some(f) = cli_output {
        return f;
    }

    // 2. config / env var value
    if let Some(f) = config_output {
        return f;
    }

    // 3. auto-detect
    if std::io::stdout().is_terminal() {
        cli::OutputFormat::Table
    } else {
        cli::OutputFormat::Tsv
    }
}

fn print_table(channels: &[slack::ChannelInfo]) {
    let name_width = channels
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let type_width = channels
        .iter()
        .map(|c| c.channel_type.as_api_str().len())
        .max()
        .unwrap_or(4)
        .max(4);

    let has_user_id = channels.iter().any(|c| c.user_id.is_some());

    let header_name: &str = "NAME";
    let header_type: &str = "TYPE";
    let header_ch_id: &str = "CHANNEL_ID";
    let header_user_id: &str = "USER_ID";

    if has_user_id {
        println!(
            "{:<name_width$}  {:<type_width$}  {:<13}  {}",
            header_name, header_type, header_ch_id, header_user_id
        );
        for ch in channels {
            println!(
                "{:<name_width$}  {:<type_width$}  {:<13}  {}",
                ch.name,
                ch.channel_type.as_api_str(),
                ch.channel_id,
                ch.user_id.as_deref().unwrap_or("")
            );
        }
    } else {
        println!(
            "{:<name_width$}  {:<type_width$}  {}",
            header_name, header_type, header_ch_id
        );
        for ch in channels {
            println!(
                "{:<name_width$}  {:<type_width$}  {}",
                ch.name,
                ch.channel_type.as_api_str(),
                ch.channel_id
            );
        }
    }
}

fn print_tsv(channels: &[slack::ChannelInfo]) {
    for ch in channels {
        println!(
            "{}\t{}\t{}\t{}",
            ch.name,
            ch.channel_type.as_api_str(),
            ch.channel_id,
            ch.user_id.as_deref().unwrap_or("")
        );
    }
}

fn print_json(channels: &[slack::ChannelInfo]) -> Result<()> {
    let json = serde_json::to_string_pretty(channels)
        .context("failed to serialize search results to JSON")?;
    println!("{json}");
    Ok(())
}

fn run_send(config: &config::Config, send: cli::SendArgs) -> Result<()> {
    let resolved = config.resolve_send()?;
    run_send_with_resolved(send, &resolved)
}

fn run_send_with_resolved(send: cli::SendArgs, resolved: &config::ResolvedConfig) -> Result<()> {
    let text_needs_stdin = send.text.as_deref() == Some("");
    let file_needs_stdin = send.file.as_deref() == Some("");

    // No flags at all → treat as implicit -t (stdin text)
    let (text, file) = if send.text.is_none() && send.file.is_none() {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            bail!("no input provided (use -t, -f, or pipe via stdin)");
        }
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        buf.truncate(buf.trim_end().len());
        (Some(buf), None)
    } else {
        // Both requesting stdin is ambiguous
        if text_needs_stdin && file_needs_stdin {
            bail!("both --text and --file require stdin; provide a value for at least one");
        }

        // Resolve file
        let file_data = match &send.file {
            Some(path) if path.is_empty() => {
                // stdin → binary
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("--file requires stdin input but stdin is a terminal");
                }
                let mut buf = Vec::new();
                stdin.lock().read_to_end(&mut buf)?;
                Some((send.filename.clone(), buf))
            }
            Some(path) => {
                // file from path
                let p = std::path::Path::new(path);
                let data =
                    std::fs::read(p).with_context(|| format!("failed to read file: {path}"))?;
                let name = p
                    .file_name()
                    .context("invalid file path")?
                    .to_string_lossy()
                    .into_owned();
                Some((name, data))
            }
            None => None,
        };

        // Resolve text
        let text = match &send.text {
            Some(t) if t.is_empty() => {
                // stdin → text
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("--text requires stdin input but stdin is a terminal");
                }
                let mut buf = String::new();
                stdin.lock().read_to_string(&mut buf)?;
                buf.truncate(buf.trim_end().len());
                Some(buf)
            }
            Some(t) => Some(t.clone()),
            None => None,
        };

        (text, file_data)
    };

    if resolved.confirm && !send.yes {
        let summary = if let Some((filename, _)) = &file {
            match text.as_deref() {
                Some(t) if !t.is_empty() => format!("file: {filename}\n> {t}"),
                _ => format!("file: {filename}"),
            }
        } else {
            let message = text.as_deref().unwrap_or("");
            format!("> {message}")
        };

        if !std::io::stdin().is_terminal() {
            bail!("confirm is enabled but stdin is not a TTY (pass -y to skip confirmation)");
        }

        if !confirm_yes_no(&format!(
            "Send to {}:\n{summary}\nSend? [y/N] ",
            resolved.channel
        ))? {
            bail!("aborted");
        }
    }

    if let Some((filename, data)) = &file {
        // max_file_size check
        if data.len() as u64 > resolved.max_file_size {
            bail!(
                "file size ({}) exceeds limit ({})",
                config::format_size(data.len() as u64),
                config::format_size(resolved.max_file_size),
            );
        }

        // For file upload, empty text means no comment
        let comment = match text.as_deref() {
            Some("") | None => None,
            Some(t) => Some(t),
        };

        slack::upload_file_bytes(&resolved.token, &resolved.channel, filename, data, comment)?;
    } else {
        // Text-only mode
        let message = text.unwrap_or_default();
        if message.is_empty() {
            bail!("message is empty");
        }
        slack::post_message(&resolved.token, &resolved.channel, &message)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_output_format_cli_flag_wins() {
        let result = resolve_output_format(
            Some(cli::OutputFormat::Json),
            Some(cli::OutputFormat::Table),
        );
        assert!(matches!(result, cli::OutputFormat::Json));
    }

    #[test]
    fn resolve_output_format_fallback_table() {
        let result = resolve_output_format(None, Some(cli::OutputFormat::Table));
        assert!(matches!(result, cli::OutputFormat::Table));
    }

    #[test]
    fn resolve_output_format_fallback_tsv() {
        let result = resolve_output_format(None, Some(cli::OutputFormat::Tsv));
        assert!(matches!(result, cli::OutputFormat::Tsv));
    }

    #[test]
    fn resolve_output_format_fallback_json() {
        let result = resolve_output_format(None, Some(cli::OutputFormat::Json));
        assert!(matches!(result, cli::OutputFormat::Json));
    }

    #[test]
    fn resolve_output_format_no_fallback_is_auto() {
        // Without a fallback, returns auto-detect result (Table or Tsv depending on TTY).
        // In test environment stdout is not a TTY, so should be Tsv.
        let result = resolve_output_format(None, None);
        assert!(matches!(result, cli::OutputFormat::Tsv));
    }
}

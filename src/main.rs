use clap::Parser;
use envault::cli::{
    resolve_db_path, resolve_password, resolve_project_name, resolve_schema_path, run_cli, Cli,
};
use envault::schema::parse_schema_file;
use envault::vault::VaultClient;

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    if args.command.is_some() {
        run_cli(args)?;
    } else {
        let db_path = resolve_db_path(args.db.as_deref())?;
        let project = resolve_project_name(args.project.as_deref())?;
        let scope = args.scope;

        // Check keychain before prompting user for password.
        // If a cached key exists, skip prompting password and initialize VaultClient with the cached key.
        let client = match VaultClient::open_with_cached_key(&db_path, &project, &scope)? {
            Some(client) => client,
            None => {
                let password = resolve_password()?;
                VaultClient::open(&db_path, &project, &scope, password.as_str())?
            }
        };

        let schema = match resolve_schema_path(None)? {
            Some(path) => parse_schema_file(&path).unwrap_or_default(),
            None => Vec::new(),
        };

        envault::tui::run(client, project, scope, schema)?;
    }
    Ok(())
}

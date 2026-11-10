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
        let password = resolve_password()?;
        let client = VaultClient::open(&db_path, &project, &scope, &password)?;

        let schema = match resolve_schema_path(None)? {
            Some(path) => parse_schema_file(&path).unwrap_or_default(),
            None => Vec::new(),
        };

        envault::tui::run(client, project, scope, schema)?;
    }
    Ok(())
}

//! The read-only commands of the check: `mssql-apply-check` (a database's
//! ConfigSave, or with `--tree` a source tree against the database's active
//! Config) and `apply-check-trees` (two XML trees).

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::sql::{SqlExec, SqlOptions};

use super::model::Verdict;

/// Exit code of a check that ran and found a restructuring, with
/// `--fail-on-restructuring`.
pub const EXIT_RESTRUCTURING: i32 = 10;

#[derive(Debug, Args)]
pub struct MssqlApplyCheckArgs {
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// The database whose ConfigSave is checked. Nothing is written.
    #[arg(long)]
    pub database: String,
    /// Check this source tree (an XML export) against the database's active
    /// Config instead of its ConfigSave: which descriptors differ and whether
    /// each difference needs the platform's own apply.
    #[arg(long)]
    pub tree: Option<PathBuf>,
    /// With --tree: the tree holds some of the objects; the ones it lacks
    /// are not removals.
    #[arg(long, requires = "tree")]
    pub partial: bool,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// XML dialect to decode the descriptors with (2.20 for 8.3, 2.21 for
    /// 8.5); by default the one the Configuration row's shape implies.
    #[arg(long)]
    pub xml_version: Option<String>,
    /// Print the verdict as JSON.
    #[arg(long)]
    pub json: bool,
    /// Exit with code 10 when the staged configuration (or the tree) needs
    /// the platform's own apply.
    #[arg(long)]
    pub fail_on_restructuring: bool,
    /// Also say what the own restructuring (S1) makes of the reasons: the
    /// operations it would do and the refusals, each with its code.
    #[arg(long)]
    pub s1: bool,
}

#[derive(Debug, Args)]
pub struct ApplyCheckTreesArgs {
    /// The tree of the configuration as it is (an export of the database).
    #[arg(long)]
    pub old: PathBuf,
    /// The tree that would replace it.
    #[arg(long)]
    pub new: PathBuf,
    /// Print the verdict as JSON.
    #[arg(long)]
    pub json: bool,
    /// Exit with code 10 when the change needs the platform's own apply.
    #[arg(long)]
    pub fail_on_restructuring: bool,
    /// Also say what the own restructuring (S1) makes of the reasons.
    #[arg(long)]
    pub s1: bool,
}

fn print(verdict: &Verdict, json: bool, s1: bool) -> Result<()> {
    let classification = s1.then(|| super::s1::classify(verdict));
    if json {
        match &classification {
            None => println!("{}", serde_json::to_string_pretty(verdict)?),
            Some(classification) => println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "verdict": verdict,
                    "s1": classification,
                }))?
            ),
        }
    } else {
        print!("{}", verdict.render_text());
        if let Some(classification) = &classification {
            print!("{}", classification.render_text());
        }
    }
    Ok(())
}

/// Runs `mssql-apply-check`; the process exit code.
pub fn run_mssql_apply_check(args: &MssqlApplyCheckArgs) -> Result<i32> {
    let password = args.sql_user.as_ref().and_then(|_| {
        args.sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok())
    });
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: true,
    })?;
    let verdict = match &args.tree {
        Some(tree) => super::check_tree_against_db(
            &sql,
            &args.database,
            tree,
            args.xml_version.as_deref(),
            args.partial,
        )?,
        None => super::check_staged(&sql, &args.database, args.xml_version.as_deref())?,
    };
    print(&verdict, args.json, args.s1)?;
    Ok(exit_code(&verdict, args.fail_on_restructuring))
}

/// Runs `apply-check-trees`; the process exit code.
pub fn run_apply_check_trees(args: &ApplyCheckTreesArgs) -> Result<i32> {
    let verdict = super::check_trees(&args.old, &args.new)?;
    print(&verdict, args.json, args.s1)?;
    Ok(exit_code(&verdict, args.fail_on_restructuring))
}

fn exit_code(verdict: &Verdict, fail: bool) -> i32 {
    if fail && verdict.needs_restructuring {
        EXIT_RESTRUCTURING
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Check {
        #[command(flatten)]
        args: MssqlApplyCheckArgs,
    }

    #[derive(Parser)]
    struct Trees {
        #[command(flatten)]
        args: ApplyCheckTreesArgs,
    }

    #[test]
    fn the_check_of_a_database_takes_a_tree_and_only_then_partial() {
        let check = Check::try_parse_from(["x", "--database", "db"]).unwrap();
        assert!(check.args.tree.is_none() && !check.args.partial);

        let check =
            Check::try_parse_from(["x", "--database", "db", "--tree", "src", "--partial"]).unwrap();
        assert_eq!(
            check.args.tree.as_deref(),
            Some(std::path::Path::new("src"))
        );
        assert!(check.args.partial);

        assert!(Check::try_parse_from(["x", "--database", "db", "--partial"]).is_err());
        assert!(!check.args.s1);
        let check = Check::try_parse_from(["x", "--database", "db", "--s1"]).unwrap();
        assert!(check.args.s1);
        assert!(Check::try_parse_from(["x"]).is_err());
    }

    #[test]
    fn the_check_of_two_trees_needs_both() {
        let trees = Trees::try_parse_from(["x", "--old", "a", "--new", "b", "--json"]).unwrap();
        assert!(trees.args.json && !trees.args.fail_on_restructuring);
        assert!(Trees::try_parse_from(["x", "--old", "a"]).is_err());
    }

    #[test]
    fn a_restructuring_is_exit_code_ten_only_when_asked_for() {
        let mut verdict = Verdict::new("rows");
        assert_eq!(exit_code(&verdict, true), 0);
        verdict.needs_restructuring = true;
        assert_eq!(exit_code(&verdict, false), 0);
        assert_eq!(exit_code(&verdict, true), EXIT_RESTRUCTURING);
    }
}

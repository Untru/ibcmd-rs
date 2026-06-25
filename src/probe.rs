use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::cli::ProbeArgs;

#[derive(Debug, Serialize)]
pub struct EnvironmentProbe {
    pub os: String,
    pub arch: String,
    pub current_dir: Option<PathBuf>,
    pub path_hits: Vec<ToolHit>,
    pub common_folder_hits: Vec<ToolHit>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ToolHit {
    pub name: String,
    pub path: PathBuf,
}

pub fn probe_environment(args: ProbeArgs) -> EnvironmentProbe {
    let names = [
        "ibcmd",
        "ibcmd.exe",
        "1cv8",
        "1cv8.exe",
        "1cv8c",
        "1cv8c.exe",
    ];
    let path_hits = find_on_path(&names).into_iter().collect();
    let common_folder_hits = if args.deep {
        find_in_common_1c_dirs(&names).into_iter().collect()
    } else {
        Vec::new()
    };

    EnvironmentProbe {
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        current_dir: env::current_dir().ok(),
        path_hits,
        common_folder_hits,
    }
}

fn find_on_path(names: &[&str]) -> BTreeSet<ToolHit> {
    let mut hits = BTreeSet::new();
    let Some(path_var) = env::var_os("PATH") else {
        return hits;
    };

    for folder in env::split_paths(&path_var) {
        for name in names {
            let candidate = folder.join(name);
            if candidate.is_file() {
                hits.insert(ToolHit {
                    name: (*name).to_string(),
                    path: candidate,
                });
            }
        }
    }

    hits
}

fn find_in_common_1c_dirs(names: &[&str]) -> BTreeSet<ToolHit> {
    let mut roots = Vec::new();
    if let Some(program_files) = env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(program_files).join("1cv8"));
    }
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files_x86).join("1cv8"));
    }

    let mut hits = BTreeSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        collect_tool_hits(&root, names, &mut hits);
    }
    hits
}

fn collect_tool_hits(root: &Path, names: &[&str], hits: &mut BTreeSet<ToolHit>) {
    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let file_name = entry.file_name().to_string_lossy();
        if names
            .iter()
            .any(|name| file_name.eq_ignore_ascii_case(name))
        {
            hits.insert(ToolHit {
                name: file_name.to_string(),
                path: entry.path().to_path_buf(),
            });
        }
    }
}

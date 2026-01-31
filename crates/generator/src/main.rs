use anyhow::{bail, Context, Result};
use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "generator", about = "Generate named runner EXEs from configure.yaml")] 
struct Opts {
    #[arg(short, long, default_value = "configure.yaml")]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Config {
    version: u32,
    #[serde(default)]
    output_dir: Option<String>,
    profiles: Vec<Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    name: String,
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    let cfg_text = fs::read_to_string(&opts.config)
        .with_context(|| format!("failed to read {}", opts.config.display()))?;
    let cfg: Config = serde_yaml::from_str(&cfg_text).context("invalid YAML")?;
    if cfg.version != 1 {
        bail!("unsupported config version: {}", cfg.version);
    }

    let name_re = Regex::new(r"^[A-Za-z0-9_-]+$").unwrap();
    for p in &cfg.profiles {
        if !name_re.is_match(&p.name) {
            bail!("invalid profile name: {}", p.name);
        }
    }

    let out_dir = cfg.output_dir.clone().unwrap_or_else(|| "dist".to_string());
    fs::create_dir_all(&out_dir).with_context(|| format!("create dir {}", out_dir))?;

    let workspace_root = workspace_root()?;
    let runner_exe = workspace_root
        .join("target")
        .join("release")
        .join(exe_name("runner"));

    if !runner_exe.exists() {
        // Try to build runner
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("-p")
            .arg("runner")
            .current_dir(&workspace_root)
            .status()
            .context("failed to spawn cargo build")?;
        if !status.success() {
            bail!("cargo build failed (runner)");
        }
    }

    for p in &cfg.profiles {
        let dest = Path::new(&out_dir).join(exe_name(&p.name));
        fs::copy(&runner_exe, &dest)
            .with_context(|| format!("copy {} -> {}", runner_exe.display(), dest.display()))?;
        println!("generated: {}", dest.display());
    }

    Ok(())
}

fn exe_name(stem: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{}.exe", stem)
    } else {
        stem.to_string()
    }
}

fn workspace_root() -> Result<PathBuf> {
    // crates/generator -> repo root
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .context("unable to resolve workspace root")?;
    Ok(root)
}


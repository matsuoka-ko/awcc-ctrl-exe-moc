use anyhow::{bail, Context, Result};
use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "generator", about = "Generate named runner EXEs from configure.yaml")] 
struct Opts {
    #[arg(short, long, default_value = "configure.yaml")]
    config: PathBuf,
    #[arg(long, help = "Skip rebuilding runner (use existing target/release/runner.exe)")]
    no_build: bool,
}

#[derive(Debug, Deserialize)]
struct Config {
    version: u32,
    #[serde(default)]
    output_dir: Option<String>,
    #[serde(default)]
    off_name: Option<String>,
    #[serde(default)]
    awcc: Option<AwccConfig>,
    profiles: Vec<Profile>,
}

#[derive(Debug, Deserialize)]
struct AwccConfig {
    path: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    start_minimized: Option<bool>,
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

    if !opts.no_build {
        // Always build runner to pick up latest changes
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
    } else if !runner_exe.exists() {
        bail!("runner executable not found: {} (remove --no-build or build manually)", runner_exe.display());
    }

    // Compute desired set from current config (profile exes + optional off exe)
    let mut desired: HashSet<String> = HashSet::new();
    for p in &cfg.profiles {
        desired.insert(exe_name(&p.name));
    }
    if let Some(off) = cfg.off_name.as_ref() {
        desired.insert(exe_name(off));
    }

    // Remove obsolete exes that were managed in previous runs but are no longer desired
    let prev = read_prev_managed(&out_dir);
    remove_obsolete(&out_dir, &prev, &desired);

    for p in &cfg.profiles {
        let dest = Path::new(&out_dir).join(exe_name(&p.name));
        fs::copy(&runner_exe, &dest)
            .with_context(|| format!("copy {} -> {}", runner_exe.display(), dest.display()))?;
        println!("generated: {}", dest.display());
    }

    // Write family file listing exe names (one per line)
    let family_path = Path::new(&out_dir).join("family.txt");
    let mut family = String::new();
    for p in &cfg.profiles {
        family.push_str(&exe_name(&p.name));
        family.push('\n');
    }
    fs::write(&family_path, family).with_context(|| format!("write {}", family_path.display()))?;
    println!("updated: {}", family_path.display());

    // Optional: generate OFF exe that stops siblings then exits
    if let Some(off) = cfg.off_name.as_ref() {
        let off_dest = Path::new(&out_dir).join(exe_name(off));
        fs::copy(&runner_exe, &off_dest)
            .with_context(|| format!("copy {} -> {}", runner_exe.display(), off_dest.display()))?;
        println!("generated: {}", off_dest.display());
        // Write off.txt with the exact EXE name
        let off_list_path = Path::new(&out_dir).join("off.txt");
        fs::write(&off_list_path, format!("{}\n", exe_name(off)))
            .with_context(|| format!("write {}", off_list_path.display()))?;
        println!("updated: {}", off_list_path.display());
    }

    // Optional: write AWCC launch settings for runner
    if let Some(awcc) = cfg.awcc.as_ref() {
        let awcc_path = Path::new(&out_dir).join("awcc_path.txt");
        fs::write(&awcc_path, format!("{}\n", awcc.path))
            .with_context(|| format!("write {}", awcc_path.display()))?;
        println!("updated: {}", awcc_path.display());

        let awcc_args = Path::new(&out_dir).join("awcc_args.txt");
        if awcc.args.is_empty() {
            if awcc_args.exists() {
                let _ = fs::remove_file(&awcc_args);
            }
        } else {
            let mut buf = String::new();
            for a in &awcc.args {
                buf.push_str(a);
                buf.push('\n');
            }
            fs::write(&awcc_args, buf).with_context(|| format!("write {}", awcc_args.display()))?;
            println!("updated: {}", awcc_args.display());
        }

        let awcc_min = Path::new(&out_dir).join("awcc_start_minimized.txt");
        let start_min = awcc.start_minimized.unwrap_or(true);
        fs::write(&awcc_min, format!("{}\n", if start_min { "true" } else { "false" }))
            .with_context(|| format!("write {}", awcc_min.display()))?;
        println!("updated: {}", awcc_min.display());
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

fn read_prev_managed(out_dir: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    // family.txt lists the profile exes
    let fam = Path::new(out_dir).join("family.txt");
    if let Ok(s) = fs::read_to_string(&fam) {
        for l in s.lines() {
            let name = l.trim();
            if !name.is_empty() && !name.starts_with('#') {
                set.insert(name.to_string());
            }
        }
    }
    // off.txt lists the off exe (single line)
    let off = Path::new(out_dir).join("off.txt");
    if let Ok(s) = fs::read_to_string(&off) {
        for l in s.lines() {
            let name = l.trim();
            if !name.is_empty() && !name.starts_with('#') {
                set.insert(name.to_string());
            }
        }
    }
    set
}

fn remove_obsolete(out_dir: &str, prev: &HashSet<String>, desired: &HashSet<String>) {
    for name in prev.difference(desired) {
        let path = Path::new(out_dir).join(name);
        if path.exists() {
            match fs::remove_file(&path) {
                Ok(_) => println!("removed: {}", path.display()),
                Err(e) => eprintln!("warn: could not remove {}: {}", path.display(), e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn exe_name_appends_exe_on_windows() {
        let n = exe_name("sample");
        assert!(n.ends_with(".exe"));
        assert!(n.starts_with("sample"));
    }

    #[test]
    fn read_prev_managed_reads_family_and_off() -> Result<()> {
        let out = unique_temp_dir()?;

        // Write family.txt
        let mut fam = fs::File::create(out.join("family.txt"))?;
        writeln!(fam, "# comment")?;
        writeln!(fam, "red.exe")?;
        writeln!(fam, " blue.exe ")?;

        // Write off.txt
        let mut off = fs::File::create(out.join("off.txt"))?;
        writeln!(off, "off.exe")?;

        let set = read_prev_managed(out.to_str().unwrap());
        assert!(set.contains("red.exe"));
        assert!(set.contains("blue.exe"));
        assert!(set.contains("off.exe"));
        fs::remove_dir_all(out)?;
        Ok(())
    }

    #[test]
    fn remove_obsolete_removes_missing_from_desired() -> Result<()> {
        let out = unique_temp_dir()?;

        // Create dummy files: old1.exe, keep.exe
        fs::write(out.join("old1.exe"), b"x")?;
        fs::write(out.join("keep.exe"), b"x")?;

        let mut prev = HashSet::new();
        prev.insert("old1.exe".to_string());
        prev.insert("keep.exe".to_string());

        let mut desired = HashSet::new();
        desired.insert("keep.exe".to_string());

        remove_obsolete(out.to_str().unwrap(), &prev, &desired);

        assert!(!out.join("old1.exe").exists());
        assert!(out.join("keep.exe").exists());
        fs::remove_dir_all(out)?;
        Ok(())
    }

    fn unique_temp_dir() -> Result<PathBuf> {
        let base = std::env::temp_dir();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = base.join(format!("awcc-ctrl-exe-moc-test-{}", nanos));
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}

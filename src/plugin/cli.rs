//! CLI-facing command handlers for `dofek-tui plugins ...`.
//!
//! These print to stdout/stderr and return a process exit code. Kept out of
//! `store` so the library half stays UI-agnostic.

use std::path::Path;

use anyhow::Result;

use super::store::PluginStore;
use crate::config::PluginsAction;

/// Dispatch a `plugins` subcommand. Returns a process exit code.
pub fn run(action: PluginsAction) -> i32 {
    match dispatch(action) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

fn dispatch(action: PluginsAction) -> Result<()> {
    let store = PluginStore::open()?;
    match action {
        PluginsAction::List => list(&store),
        PluginsAction::Add { path, args } => add(&store, &path, args),
        PluginsAction::Remove { name } => remove(&store, &name),
        PluginsAction::Enable { name } => set_enabled(&store, &name, true),
        PluginsAction::Disable { name } => set_enabled(&store, &name, false),
    }
}

fn list(store: &PluginStore) -> Result<()> {
    let plugins = store.list()?;
    if plugins.is_empty() {
        println!("No plugins installed.");
        println!();
        println!("Add one with:");
        println!("  dofek-tui plugins add <path-to-binary>");
        println!();
        println!(
            "Managed dir: {}",
            store.plugins_dir().display()
        );
        return Ok(());
    }
    println!(
        "{:<24} {:<10} {:<8} DESCRIPTION",
        "NAME", "VERSION", "ENABLED"
    );
    for p in plugins {
        let version = if p.version.is_empty() { "-" } else { &p.version };
        let enabled = if p.enabled { "yes" } else { "no" };
        let desc = if p.description.is_empty() {
            "(no description)"
        } else {
            &p.description
        };
        println!("{:<24} {:<10} {:<8} {}", p.name, version, enabled, desc);
    }
    println!();
    println!("Managed dir: {}", store.plugins_dir().display());
    Ok(())
}

fn add(store: &PluginStore, path: &Path, args: Vec<String>) -> Result<()> {
    let installed = store.add(path, args)?;
    println!("✓ installed {} v{}", installed.name, version_or_unknown(&installed.version));
    println!("  binary: {}", installed.binary_path.display());
    if !installed.description.is_empty() {
        println!("  about:  {}", installed.description);
    }
    if !installed.args.is_empty() {
        println!("  args:   {}", installed.args.join(" "));
    }
    println!();
    println!(
        "Restart dofek (or the GUI) to load the new plugin. Use `dofek-tui plugins disable {}` to turn it off without uninstalling.",
        installed.name
    );
    Ok(())
}

fn remove(store: &PluginStore, name: &str) -> Result<()> {
    store.remove(name)?;
    println!("✓ removed {name}");
    Ok(())
}

fn set_enabled(store: &PluginStore, name: &str, enabled: bool) -> Result<()> {
    store.set_enabled(name, enabled)?;
    println!(
        "✓ {} {name}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn version_or_unknown(v: &str) -> &str {
    if v.is_empty() { "?" } else { v }
}

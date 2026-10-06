//! `crumb skill`: install or print the agent skill that ships inside the binary.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// The skill text, embedded so it always matches the binary that installs it.
pub const SKILL: &str = include_str!("../skill/SKILL.md");

/// Writes `~/.agents/skills/crumb/SKILL.md`, then links it into `~/.claude/skills`.
/// Returns the lines to show.
pub fn install(home: &Path) -> Result<Vec<String>, String> {
    let dir = home.join(".agents/skills/crumb");
    let file = dir.join("SKILL.md");
    let mut said = Vec::new();
    fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    if write_if_changed(&file, SKILL)? {
        said.push(format!("Installed the crumb skill: {}", file.display()));
    } else {
        said.push(format!(
            "The crumb skill is already current: {}",
            file.display()
        ));
    }
    said.push(link_claude(home, &dir)?);
    Ok(said)
}

/// Replaces `path` with `text` atomically; false when it already held `text`.
fn write_if_changed(path: &Path, text: &str) -> Result<bool, String> {
    if fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, text).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("replacing {}: {e}", path.display()))?;
    Ok(true)
}

/// Points `~/.claude/skills/crumb` at the skill directory. Skipped when Claude Code has never
/// run here, and never overwrites something that isn't our link.
fn link_claude(home: &Path, dir: &Path) -> Result<String, String> {
    let claude = home.join(".claude");
    if !claude.is_dir() {
        return Ok(format!(
            "Skipped the Claude Code link: {} not found",
            claude.display()
        ));
    }
    let skills = claude.join("skills");
    fs::create_dir_all(&skills).map_err(|e| format!("creating {}: {e}", skills.display()))?;
    let link = skills.join("crumb");
    match fs::symlink_metadata(&link) {
        Ok(_) if fs::canonicalize(&link).ok() == fs::canonicalize(dir).ok() => {
            Ok(format!("Claude Code link is in place: {}", link.display()))
        }
        Ok(_) => Ok(format!(
            "{} already exists and is not a link to {}; left alone",
            link.display(),
            dir.display()
        )),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            link_dir(dir, &link).map_err(|e| format!("linking {}: {e}", link.display()))?;
            Ok(format!("Linked {} -> {}", link.display(), dir.display()))
        }
        Err(e) => Err(format!("checking {}: {e}", link.display())),
    }
}

#[cfg(unix)]
fn link_dir(dir: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(dir, link)
}

#[cfg(not(unix))]
fn link_dir(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("links need Unix"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn embedded_skill_has_frontmatter() {
        assert!(SKILL.starts_with("---\nname: crumb\ndescription: "));
    }

    /// The skill must not teach commands or flags the binary doesn't have.
    #[test]
    fn every_command_the_skill_shows_exists() {
        let cli = crate::Cli::command();
        let mut checked = 0;
        for line in SKILL.lines() {
            let Some(rest) = line.trim().strip_prefix("crumb ") else {
                continue;
            };
            let word = rest.split_whitespace().next().unwrap_or("");
            if word.starts_with('-') || word.starts_with('[') {
                continue;
            }
            assert!(
                cli.find_subcommand(word).is_some(),
                "the skill shows `crumb {word}`, which isn't a command"
            );
            checked += 1;
        }
        assert!(checked > 10, "found only {checked} commands in the skill");
    }

    #[test]
    fn install_is_idempotent_and_links_claude() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join(".claude")).unwrap();
        install(home.path()).unwrap();
        let again = install(home.path()).unwrap();
        assert!(again[0].contains("already current") && again[1].contains("in place"));
        let file = home.path().join(".agents/skills/crumb/SKILL.md");
        assert_eq!(fs::read_to_string(file).unwrap(), SKILL);
        assert!(home.path().join(".claude/skills/crumb/SKILL.md").is_file());
    }

    #[test]
    fn install_skips_claude_when_absent_and_keeps_foreign_dirs() {
        let home = tempfile::tempdir().unwrap();
        install(home.path()).unwrap();
        assert!(!home.path().join(".claude").exists());

        let foreign = home.path().join(".claude/skills/crumb");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("SKILL.md"), "mine").unwrap();
        let said = install(home.path()).unwrap();
        assert!(said[1].contains("left alone"));
        assert_eq!(
            fs::read_to_string(foreign.join("SKILL.md")).unwrap(),
            "mine"
        );
    }
}

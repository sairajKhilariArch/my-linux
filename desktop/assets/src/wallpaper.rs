//! Wallpaper discovery, library, recorded state and wallpaper tools.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use config_manager::install_content;

use crate::AssetError;

/// Image formats My Linux can read for previews and palette extraction.
pub const WALLPAPER_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// How deep the library scan descends into a directory.
const MAX_DEPTH: usize = 3;

/// True when `path` looks like a wallpaper My Linux can read.
pub fn is_wallpaper(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            WALLPAPER_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallpaper {
    /// File stem, which is what the CLI accepts as a wallpaper name.
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// A set of directories searched for wallpapers.
#[derive(Debug, Clone)]
pub struct WallpaperLibrary {
    dirs: Vec<PathBuf>,
}

impl WallpaperLibrary {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    /// `$MY_LINUX_WALLPAPERS` (colon separated) or the XDG wallpaper folders.
    pub fn from_environment() -> Self {
        if let Some(list) = env::var_os("MY_LINUX_WALLPAPERS") {
            return Self::new(env::split_paths(&list).collect());
        }

        let mut dirs = vec![data_home().join("my-linux").join("wallpapers")];

        if let Some(home) = home_dir() {
            let pictures = home.join("Pictures").join("Wallpapers");

            if pictures.is_dir() {
                dirs.push(pictures);
            }
        }

        Self::new(dirs)
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// The first library directory, which receives installed wallpapers.
    pub fn install_dir(&self) -> Option<&Path> {
        self.dirs.first().map(PathBuf::as_path)
    }

    /// Every wallpaper in the library, sorted by name.
    pub fn list(&self) -> Vec<Wallpaper> {
        let mut found = Vec::new();

        for directory in &self.dirs {
            collect(directory, 0, &mut found);
        }

        found.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.path.cmp(&right.path))
        });
        found.dedup_by(|left, right| left.path == right.path);

        found
    }

    /// Resolves a name (`neon`, `neon.png`) or an existing file path.
    pub fn find(&self, query: &str) -> Option<Wallpaper> {
        let path = PathBuf::from(query);

        if path.is_file() {
            return Some(Wallpaper {
                name: stem(&path),
                bytes: fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0),
                path,
            });
        }

        let query = query.trim().to_lowercase();

        if query.is_empty() {
            return None;
        }

        let wallpapers = self.list();
        let with_extension = |wallpaper: &Wallpaper| {
            wallpaper
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_lowercase)
                .is_some_and(|name| name == query)
        };

        wallpapers
            .iter()
            .find(|wallpaper| with_extension(wallpaper))
            .or_else(|| {
                wallpapers
                    .iter()
                    .find(|wallpaper| wallpaper.name.to_lowercase() == query)
            })
            .cloned()
    }

    /// Copies `source` into the library unless it is already inside it.
    pub fn install(&self, source: &Path) -> Result<Wallpaper, AssetError> {
        if !source.is_file() {
            return Err(AssetError::Invalid(format!(
                "{} is not a file",
                source.display()
            )));
        }

        if self.dirs.iter().any(|dir| source.starts_with(dir)) {
            return Ok(Wallpaper {
                name: stem(source),
                bytes: fs::metadata(source).map(|meta| meta.len()).unwrap_or(0),
                path: source.to_path_buf(),
            });
        }

        let directory = self
            .install_dir()
            .ok_or_else(|| AssetError::Invalid("No wallpaper library is configured".to_string()))?;

        fs::create_dir_all(directory)?;

        let name = source
            .file_name()
            .ok_or_else(|| AssetError::Invalid("The wallpaper has no file name".to_string()))?;

        let target = directory.join(name);

        if !target.exists() {
            fs::copy(source, &target)?;
        }

        Ok(Wallpaper {
            name: stem(&target),
            bytes: fs::metadata(&target).map(|meta| meta.len()).unwrap_or(0),
            path: target,
        })
    }
}

fn collect(directory: &Path, depth: usize, found: &mut Vec<Wallpaper>) {
    if depth > MAX_DEPTH {
        return;
    }

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect(&path, depth + 1, found);
        } else if is_wallpaper(&path) {
            found.push(Wallpaper {
                name: stem(&path),
                bytes: entry.metadata().map(|meta| meta.len()).unwrap_or(0),
                path,
            });
        }
    }
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

fn write_or_remove(file: PathBuf, path: Option<&Path>) -> Result<(), AssetError> {
    match path {
        Some(path) => {
            install_content(path.to_string_lossy().as_bytes(), &file).map_err(AssetError::from)
        }
        None => match fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        },
    }
}

/// The wallpaper recorded under an installation root.
#[derive(Debug, Clone)]
pub struct WallpaperState {
    root: PathBuf,
}

impl WallpaperState {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn current_file(&self) -> PathBuf {
        self.root.join("state").join("wallpaper")
    }

    fn previous_file(&self) -> PathBuf {
        self.root.join("state").join("wallpaper-previous")
    }

    fn history_file(&self) -> PathBuf {
        self.root.join("state").join("wallpaper-history.log")
    }

    pub fn current(&self) -> Option<PathBuf> {
        read_path(&self.current_file())
    }

    pub fn previous(&self) -> Option<PathBuf> {
        read_path(&self.previous_file())
    }

    /// Restores the recorded current and previous wallpapers without adding a
    /// history entry. This undoes the state change of a failed operation.
    pub fn restore_state(
        &self,
        current: Option<&Path>,
        previous: Option<&Path>,
    ) -> Result<(), AssetError> {
        write_or_remove(self.current_file(), current)?;
        write_or_remove(self.previous_file(), previous)
    }

    /// Records `path` as the active wallpaper and keeps the old one as previous.
    pub fn set(&self, path: &Path, action: &str) -> Result<(), AssetError> {
        let current = self.current();

        if let Some(current) = &current
            && current.as_path() != path
        {
            install_content(current.to_string_lossy().as_bytes(), self.previous_file())?;
        }

        install_content(path.to_string_lossy().as_bytes(), self.current_file())?;
        self.append_history(action, path)
    }

    fn append_history(&self, action: &str, path: &Path) -> Result<(), AssetError> {
        let mut content = format!("{}\t{}\t{}\n", timestamp(), action, path.display());

        if let Ok(existing) = fs::read_to_string(self.history_file()) {
            content = format!("{existing}{content}");
        }

        install_content(content.as_bytes(), self.history_file())?;

        Ok(())
    }

    /// Every recorded wallpaper change, oldest first.
    pub fn history(&self) -> Vec<WallpaperHistoryEntry> {
        fs::read_to_string(self.history_file())
            .map(|contents| {
                contents
                    .lines()
                    .filter_map(|line| {
                        let mut fields = line.split('\t');

                        let timestamp = fields.next()?.parse().ok()?;
                        let action = fields.next()?.to_string();
                        let path = fields.next()?.to_string();

                        Some(WallpaperHistoryEntry {
                            timestamp,
                            action,
                            path,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperHistoryEntry {
    pub timestamp: u64,
    pub action: String,
    pub path: String,
}

fn read_path(file: &Path) -> Option<PathBuf> {
    fs::read_to_string(file)
        .ok()
        .map(|contents| PathBuf::from(contents.trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// A wallpaper daemon My Linux knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperTool {
    Swww,
    Hyprpaper,
}

impl WallpaperTool {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Swww => "swww",
            Self::Hyprpaper => "hyprpaper",
        }
    }
}

/// Looks for an executable on `PATH`.
pub(crate) fn program_exists(program: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path)
        .map(|directory| directory.join(program))
        .any(|candidate| candidate.is_file())
}

/// The wallpaper daemon to use, if one is installed.
pub fn detect_tool() -> Option<WallpaperTool> {
    if program_exists("swww") {
        Some(WallpaperTool::Swww)
    } else if program_exists("hyprpaper") && program_exists("hyprctl") {
        Some(WallpaperTool::Hyprpaper)
    } else {
        None
    }
}

/// Sets the wallpaper with `tool` and returns the commands that were run.
pub fn set_wallpaper(tool: WallpaperTool, path: &Path) -> Result<Vec<String>, AssetError> {
    let target = path.display().to_string();

    let commands: Vec<Vec<String>> = match tool {
        WallpaperTool::Swww => vec![vec!["swww".to_string(), "img".to_string(), target]],
        WallpaperTool::Hyprpaper => vec![
            vec![
                "hyprctl".to_string(),
                "hyprpaper".to_string(),
                "preload".to_string(),
                target.clone(),
            ],
            vec![
                "hyprctl".to_string(),
                "hyprpaper".to_string(),
                "wallpaper".to_string(),
                format!(",{target}"),
            ],
        ],
    };

    let mut executed = Vec::new();

    for command in commands {
        let Some((program, arguments)) = command.split_first() else {
            continue;
        };

        let status = Command::new(program)
            .args(arguments)
            .status()
            .map_err(|error| AssetError::Tool(format!("{program}: {error}")))?;

        if !status.success() {
            return Err(AssetError::Tool(format!(
                "`{}` exited with {}",
                command.join(" "),
                status.code().unwrap_or(-1)
            )));
        }

        executed.push(command.join(" "));
    }

    Ok(executed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "my-linux-wallpaper-{}-{}",
            name,
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    #[test]
    fn restores_recorded_state_without_touching_history() {
        let directory = scratch("restore-state");
        let first = directory.join("first.png");
        let second = directory.join("second.png");

        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();

        let state = WallpaperState::new(&directory);

        state.set(&first, "apply").unwrap();
        state.set(&second, "apply").unwrap();

        let history = state.history().len();

        state.restore_state(Some(&first), Some(&first)).unwrap();

        assert_eq!(state.current(), Some(first.clone()));
        assert_eq!(state.previous(), Some(first.clone()));
        assert_eq!(state.history().len(), history);

        state.restore_state(None, None).unwrap();

        assert_eq!(state.current(), None);
        assert_eq!(state.previous(), None);
    }

    #[test]
    fn discovers_wallpapers_recursively_and_ignores_other_files() {
        let directory = scratch("discover");
        let nested = directory.join("nested");

        fs::create_dir_all(&nested).unwrap();
        fs::write(directory.join("b.png"), "x").unwrap();
        fs::write(directory.join("a.JPG"), "x").unwrap();
        fs::write(nested.join("c.jpeg"), "x").unwrap();
        fs::write(nested.join("notes.txt"), "x").unwrap();

        let library = WallpaperLibrary::new(vec![directory.clone()]);
        let wallpapers = library.list();

        assert_eq!(wallpapers.len(), 3);
        assert_eq!(wallpapers[0].name, "a");
        assert_eq!(wallpapers[1].name, "b");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn finds_wallpapers_by_name_extension_or_path() {
        let directory = scratch("find");
        let file = directory.join("neon.png");

        fs::write(&file, "x").unwrap();

        let library = WallpaperLibrary::new(vec![directory.clone()]);

        assert_eq!(library.find("neon").map(|w| w.path), Some(file.clone()));
        assert_eq!(library.find("neon.png").map(|w| w.path), Some(file.clone()));
        assert_eq!(
            library.find(file.to_str().unwrap()).map(|w| w.path),
            Some(file.clone())
        );
        assert!(library.find("missing").is_none());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn installs_wallpapers_into_the_library() {
        let source_dir = scratch("install-source");
        let library_dir = scratch("install-library");
        let source = source_dir.join("wall.png");

        fs::write(&source, "pixels").unwrap();

        let library = WallpaperLibrary::new(vec![library_dir.clone()]);
        let installed = library.install(&source).unwrap();

        assert_eq!(installed.path, library_dir.join("wall.png"));
        assert_eq!(fs::read_to_string(&installed.path).unwrap(), "pixels");

        // Installing a wallpaper that already lives in the library is a no-op.
        let again = library.install(&installed.path).unwrap();
        assert_eq!(again.path, installed.path);

        fs::remove_dir_all(source_dir).unwrap();
        fs::remove_dir_all(library_dir).unwrap();
    }

    #[test]
    fn records_current_previous_and_history() {
        let directory = scratch("state");
        let state = WallpaperState::new(directory.clone());

        assert!(state.current().is_none());
        assert!(state.previous().is_none());

        state
            .set(Path::new("/wallpapers/first.png"), "apply")
            .unwrap();
        assert_eq!(
            state.current(),
            Some(PathBuf::from("/wallpapers/first.png"))
        );
        assert!(state.previous().is_none());

        state
            .set(Path::new("/wallpapers/second.png"), "apply")
            .unwrap();
        assert_eq!(
            state.current(),
            Some(PathBuf::from("/wallpapers/second.png"))
        );
        assert_eq!(
            state.previous(),
            Some(PathBuf::from("/wallpapers/first.png"))
        );

        let history = state.history();

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].action, "apply");
        assert!(history[1].path.ends_with("second.png"));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn detects_programs_on_path() {
        assert!(!program_exists("my-linux-wallpaper-tool-does-not-exist"));
    }
}

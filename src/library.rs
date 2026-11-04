use crate::engine::err;
use crate::i18n::tr;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: u64,
    pub parent: u64,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: u64,
    pub folder: u64,
    pub path: String,
    pub title: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub folders: Vec<Folder>,
    pub episodes: Vec<Episode>,
    pub queue: Vec<String>,
    pub current: usize,
}

pub struct Library {
    pub data: Data,
    pub folder: u64,
    pub folder_name: String,
    pub error: Option<String>,
    path: PathBuf,
    _lock: Option<fs::File>,
    last_queue: Vec<String>,
}

impl Library {
    pub fn load(config: &Path) -> Self {
        let locked = fs::create_dir_all(config).and_then(|_| {
            let mut options = fs::OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(0);
            }
            let file = options.open(config.join("biblioteca.lock"))?;
            #[cfg(unix)]
            file.try_lock().map_err(std::io::Error::from)?;
            Ok(file)
        });
        let (lock, lock_error) = match locked {
            Ok(lock) => (Some(lock), None),
            Err(error) => (
                None,
                Some(format!(
                    "{} {error}",
                    tr("A biblioteca está aberta em outra janela ou sem permissão de gravação.")
                )),
            ),
        };
        let path = config.join("biblioteca.json");
        let loaded = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Data>(&bytes).map_err(err),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
            Err(error) => Err(err(error)),
        };
        let (data, error) = match loaded {
            Ok(data) if valid(&data) => (data, None),
            Ok(_) => (
                Data::default(),
                Some(
                    tr("Estrutura da biblioteca inválida. O arquivo original foi preservado.")
                        .into(),
                ),
            ),
            Err(error) => (
                Data::default(),
                Some(format!(
                    "Falha ao ler a biblioteca. O arquivo original foi preservado. {error}"
                )),
            ),
        };
        Self {
            last_queue: data.queue.clone(),
            data,
            folder: 0,
            folder_name: String::new(),
            error: error.or(lock_error),
            path,
            _lock: lock,
        }
    }

    pub fn save(&mut self) {
        // A read error must never silently overwrite an existing library.
        if self.error.is_some() {
            return;
        }
        let result = (|| {
            fs::create_dir_all(
                self.path
                    .parent()
                    .ok_or(tr("Pasta de configuração inválida."))?,
            )
            .map_err(err)?;
            let bytes = serde_json::to_vec_pretty(&self.data).map_err(err)?;
            let pending = self
                .path
                .with_extension(format!("{}.tmp", std::process::id()));
            fs::write(&pending, bytes).map_err(err)?;
            if let Err(error) = fs::rename(&pending, &self.path) {
                let _ = fs::remove_file(pending);
                return Err(err(error));
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            self.error = Some(format!(
                "{} {error}",
                tr("Não foi possível salvar a biblioteca.")
            ));
        }
    }

    fn next_id(&self) -> u64 {
        self.data
            .folders
            .iter()
            .map(|f| f.id)
            .chain(self.data.episodes.iter().map(|e| e.id))
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn create_folder(&mut self) {
        let name = self.folder_name.trim();
        if name.is_empty() || self.error.is_some() {
            return;
        }
        let folder = Folder {
            id: self.next_id(),
            parent: self.folder,
            name: name.into(),
        };
        self.data.folders.push(folder);
        self.folder_name.clear();
        self.save();
    }

    pub fn add(&mut self, paths: impl IntoIterator<Item = String>) {
        if self.error.is_some() {
            return;
        }
        let mut changed = false;
        for path in paths {
            if self
                .data
                .episodes
                .iter()
                .any(|episode| episode.folder == self.folder && episode.path == path)
            {
                continue;
            }
            let title = Path::new(&path)
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or(&path)
                .to_owned();
            self.data.episodes.push(Episode {
                id: self.next_id(),
                folder: self.folder,
                path,
                title,
            });
            changed = true;
        }
        if changed {
            self.save();
        }
    }

    pub fn sync_queue(&mut self, playlist: &serde_json::Value, position: &serde_json::Value) {
        let Some(items) = playlist.as_array() else {
            return;
        };
        let paths: Vec<String> = items
            .iter()
            .filter_map(|item| item["filename"].as_str().map(str::to_owned))
            .collect();
        if paths.is_empty() {
            return;
        }
        let mut changed = false;
        if paths != self.last_queue {
            let new_paths: Vec<_> = paths
                .iter()
                .filter(|path| {
                    !self
                        .data
                        .episodes
                        .iter()
                        .any(|episode| &episode.path == *path)
                })
                .cloned()
                .collect();
            self.add(new_paths);
            self.data.queue = paths.clone();
            self.last_queue = paths;
            changed = true;
        }
        if let Some(position) = position.as_u64() {
            let position = position as usize;
            if self.data.current != position {
                self.data.current = position;
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    pub fn title(&self) -> String {
        self.folder_title(self.folder)
    }

    pub fn folder_title(&self, id: u64) -> String {
        let mut names = Vec::new();
        let mut current = id;
        while let Some(folder) = self.data.folders.iter().find(|folder| folder.id == current) {
            names.push(folder.name.clone());
            current = folder.parent;
        }
        names.reverse();
        if names.is_empty() {
            tr("Todos os animes").into()
        } else {
            names.join(" / ")
        }
    }

    pub fn parent(&self) -> u64 {
        self.data
            .folders
            .iter()
            .find(|folder| folder.id == self.folder)
            .map_or(0, |folder| folder.parent)
    }

    pub fn remove_folder(&mut self, id: u64) {
        if self.error.is_some() || id == 0 {
            return;
        }
        let Some(parent) = self
            .data
            .folders
            .iter()
            .find(|folder| folder.id == id)
            .map(|folder| folder.parent)
        else {
            return;
        };
        let mut removed = std::collections::HashSet::from([id]);
        loop {
            let before = removed.len();
            for folder in &self.data.folders {
                if removed.contains(&folder.parent) {
                    removed.insert(folder.id);
                }
            }
            if removed.len() == before {
                break;
            }
        }
        self.data
            .folders
            .retain(|folder| !removed.contains(&folder.id));
        for episode in &mut self.data.episodes {
            if removed.contains(&episode.folder) {
                episode.folder = parent;
            }
        }
        let mut seen = std::collections::HashSet::new();
        self.data
            .episodes
            .retain(|episode| seen.insert((episode.folder, episode.path.clone())));
        if removed.contains(&self.folder) {
            self.folder = parent;
        }
        self.save();
    }

    pub fn move_episode(&mut self, id: u64, folder: u64) {
        if self.error.is_some() {
            return;
        }
        if let Some(episode) = self.data.episodes.iter_mut().find(|e| e.id == id) {
            episode.folder = folder;
        }
        let mut seen = std::collections::HashSet::new();
        self.data
            .episodes
            .retain(|episode| seen.insert((episode.folder, episode.path.clone())));
        self.save();
    }

    pub fn remove_episode(&mut self, id: u64) {
        if self.error.is_some() {
            return;
        }
        self.data.episodes.retain(|episode| episode.id != id);
        self.save();
    }

    pub fn restore_list(&self, config: &Path) -> Result<Option<PathBuf>, String> {
        if self.data.queue.is_empty() {
            return Ok(None);
        }
        fs::create_dir_all(config).map_err(err)?;
        let file = config.join(format!("biblioteca-fila-{}.m3u8", std::process::id()));
        let mut content = String::from(
            "#EXTM3U
",
        );
        for path in &self.data.queue {
            if path.contains(char::from(13)) || path.contains(char::from(10)) {
                continue;
            }
            content.push_str(path);
            content.push(char::from(10));
        }
        fs::write(&file, content).map_err(err)?;
        Ok(Some(file))
    }
}

fn valid(data: &Data) -> bool {
    let mut ids = std::collections::HashSet::new();
    for folder in &data.folders {
        if folder.id == 0 || !ids.insert(folder.id) {
            return false;
        }
        let mut ancestors = std::collections::HashSet::new();
        let mut current = folder.id;
        while current != 0 {
            if !ancestors.insert(current) {
                return false;
            }
            let Some(parent) = data.folders.iter().find(|folder| folder.id == current) else {
                return false;
            };
            current = parent.parent;
        }
    }
    data.episodes.iter().all(|episode| {
        episode.id != 0
            && ids.insert(episode.id)
            && (episode.folder == 0 || data.folders.iter().any(|f| f.id == episode.folder))
    })
}

pub fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_a_folder_moves_nested_episodes_to_parent_and_persists() {
        let config = std::env::temp_dir().join(format!("ani-library-{}", nonce()));
        let mut library = Library::load(&config);
        library.folder_name = "Anime".into();
        library.create_folder();
        let anime = library.data.folders[0].id;
        library.folder = anime;
        library.folder_name = "Temporada".into();
        library.create_folder();
        let season = library.data.folders[1].id;
        library.add(["episode.mp4".into()]);
        library.folder = season;
        library.folder_name = "Extras".into();
        library.create_folder();
        let extras = library.data.folders[2].id;
        library.add(["episode.mp4".into()]);
        library.folder = extras;
        library.add(["extra.mp4".into()]);
        library.remove_folder(season);
        assert_eq!(library.folder, anime);
        assert_eq!(library.data.folders.len(), 1);
        assert_eq!(library.data.episodes.len(), 2);
        assert!(
            library
                .data
                .episodes
                .iter()
                .all(|episode| episode.folder == anime)
        );
        assert!(valid(&library.data));
        drop(library);
        let restored = Library::load(&config);
        assert_eq!(restored.data.folders.len(), 1);
        assert_eq!(restored.data.episodes.len(), 2);
        drop(restored);
        fs::remove_file(config.join("biblioteca.json")).unwrap();
        fs::remove_file(config.join("biblioteca.lock")).unwrap();
        fs::remove_dir(config).unwrap();
    }
    #[test]
    fn an_empty_engine_snapshot_does_not_erase_the_saved_queue() {
        let config = std::env::temp_dir().join(format!("ani-library-{}", nonce()));
        let mut library = Library::load(&config);
        library.sync_queue(
            &serde_json::json!([{"filename":"episode.mp4"}]),
            &serde_json::json!(0),
        );
        library.sync_queue(&serde_json::json!([]), &serde_json::Value::Null);
        assert_eq!(library.data.queue, ["episode.mp4"]);
        drop(library);
        fs::remove_file(config.join("biblioteca.json")).unwrap();
        fs::remove_file(config.join("biblioteca.lock")).unwrap();
        fs::remove_dir(config).unwrap();
    }

    #[test]
    fn second_window_cannot_overwrite_the_active_library() {
        let config = std::env::temp_dir().join(format!("ani-library-{}", nonce()));
        let mut first = Library::load(&config);
        first.add(["episode.mp4".into()]);
        let mut second = Library::load(&config);
        assert!(second.error.is_some());
        second.add(["other.mp4".into()]);
        assert_eq!(second.data.episodes.len(), 1);
        drop(second);
        drop(first);
        fs::remove_file(config.join("biblioteca.json")).unwrap();
        fs::remove_file(config.join("biblioteca.lock")).unwrap();
        fs::remove_dir(config).unwrap();
    }
    #[test]
    fn saves_nested_folders_episodes_and_queue_across_restart() {
        let config = std::env::temp_dir().join(format!("ani-library-{}", nonce()));
        let mut library = Library::load(&config);
        library.folder_name = "Anime".into();
        library.create_folder();
        library.folder = library.data.folders[0].id;
        library.folder_name = "Temporada 2".into();
        library.create_folder();
        library.folder = library.data.folders[1].id;
        library.add(["F:/vídeos/Episódio 1.mkv".into()]);
        library.sync_queue(
            &serde_json::json!([{"filename":"F:/vídeos/Episódio 1.mkv"}]),
            &serde_json::json!(0),
        );
        drop(library);
        let restored = Library::load(&config);
        assert!(restored.error.is_none());
        assert_eq!(restored.data.folders.len(), 2);
        assert_eq!(restored.data.episodes.len(), 1);
        assert_eq!(restored.data.queue.len(), 1);
        assert_eq!(
            restored.folder_title(restored.data.episodes[0].folder),
            "Anime / Temporada 2"
        );
        drop(restored);
        fs::remove_file(config.join("biblioteca.json")).unwrap();
        fs::remove_file(config.join("biblioteca.lock")).unwrap();
        fs::remove_dir(config).unwrap();
    }

    #[test]
    fn corrupt_library_is_preserved() {
        let config = std::env::temp_dir().join(format!("ani-library-{}", nonce()));
        fs::create_dir(&config).unwrap();
        let file = config.join("biblioteca.json");
        fs::write(&file, "invalid").unwrap();
        let mut library = Library::load(&config);
        library.add(["episode.mp4".into()]);
        assert_eq!(fs::read_to_string(&file).unwrap(), "invalid");
        drop(library);
        fs::remove_file(file).unwrap();
        fs::remove_file(config.join("biblioteca.lock")).unwrap();
        fs::remove_dir(config).unwrap();
    }
}

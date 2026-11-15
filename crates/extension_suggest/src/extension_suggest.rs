use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use path::rel_path::RelPath;

const SUGGESTIONS_BY_EXTENSION_ID: &[(&str, &[&str])] = &[
    // Phocida - Pascal IDE: только расширения для Pascal и связанных файлов
    ("env", &[".env", ".envrc", "env", "envrc"]),
    (
        "git-firefly",
        &[
            ".containerignore",
            ".cursorignore",
            ".dockerignore",
            ".eslintignore",
            ".fdignore",
            ".git-blame-ignore-revs",
            ".gitattributes",
            ".gitconfig",
            ".gitignore",
            ".gitignore_global",
            ".gitmodules",
            ".ignore",
            ".lfsconfig",
            ".npmignore",
            ".prettierignore",
            ".rgignore",
            ".vscodeignore",
            "config.worktree",
            "git-rebase-todo",
            "gitattributes",
        ],
    ),
    // TODO: Добавить расширение для Pascal если будет создано позже
    // ("pascal", &["pas", "pp", "lpr", "inc", "lfm", "lpi", "lpk", "dpr", "dpk"]),
];

pub struct LanguageAdditionSuggestion {
    pub extension_id: &'static str,
    pub languages: &'static [&'static str],
    pub title: &'static str,
    pub description: &'static str,
    pub docs_url: &'static str,
    pub install_message: &'static str,
}

const SUGGESTIONS_BY_LANGUAGE: &[LanguageAdditionSuggestion] = &[];

fn suggested_extensions() -> &'static HashMap<&'static str, Arc<str>> {
    static SUGGESTIONS_BY_PATH_SUFFIX: OnceLock<HashMap<&str, Arc<str>>> = OnceLock::new();
    SUGGESTIONS_BY_PATH_SUFFIX.get_or_init(|| {
        SUGGESTIONS_BY_EXTENSION_ID
            .iter()
            .flat_map(|(name, path_suffixes)| {
                let name = Arc::<str>::from(*name);
                path_suffixes
                    .iter()
                    .map(move |suffix| (*suffix, name.clone()))
            })
            .collect()
    })
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SuggestedExtension {
    pub extension_id: Arc<str>,
    pub file_name_or_extension: Arc<str>,
}

/// Returns the suggested extension for the given [`Path`].
pub fn suggest_extension(path: &RelPath) -> Option<SuggestedExtension> {
    let (file_name_or_extension, extension_id) = path
        .file_name()
        .and_then(|file_name| {
            // We suggest against file names first, as these suggestions will be more
            // specific than ones based on the file extension.
            suggested_extensions()
                .get(file_name)
                .map(|suggestion| (Arc::from(file_name), suggestion))
        })
        .or_else(|| {
            path.extension().and_then(|file_extension| {
                suggested_extensions()
                    .get(file_extension)
                    .map(|suggestion| (Arc::from(file_extension), suggestion))
            })
        })?;

    Some(SuggestedExtension {
        extension_id: extension_id.clone(),
        file_name_or_extension,
    })
}

pub fn additional_suggestion_for_language(
    language_name: &str,
) -> Option<&LanguageAdditionSuggestion> {
    SUGGESTIONS_BY_LANGUAGE
        .iter()
        .find(|suggestion| suggestion.languages.contains(&language_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use path::rel_path::rel_path;

    #[test]
    pub fn test_suggested_extension() {
        assert_eq!(
            suggest_extension(rel_path("a/b/c/d/.gitignore")),
            Some(SuggestedExtension {
                extension_id: "git-firefly".into(),
                file_name_or_extension: ".gitignore".into()
            })
        );
        // Dotfiles have no `Path::extension`, so they match by name.
        assert_eq!(
            suggest_extension(rel_path(".envrc")),
            Some(SuggestedExtension {
                extension_id: "env".into(),
                file_name_or_extension: ".envrc".into()
            })
        );
        assert_eq!(
            suggest_extension(rel_path(".gitattributes")),
            Some(SuggestedExtension {
                extension_id: "git-firefly".into(),
                file_name_or_extension: ".gitattributes".into()
            })
        );
    }

    #[test]
    pub fn suggested_path_suffixes_are_unique() {
        let mut claims: HashMap<&str, &str> = HashMap::new();
        for (extension_id, path_suffixes) in SUGGESTIONS_BY_EXTENSION_ID {
            for suffix in *path_suffixes {
                let previous = claims.insert(suffix, extension_id);
                assert!(
                    previous.is_none(),
                    "duplicate suffix `{suffix}` is claimed by both `{}` and `{extension_id}`",
                    previous.unwrap()
                );
            }
        }
    }

    #[test]
    pub fn table_is_sorted_by_extension_id() {
        assert!(
            SUGGESTIONS_BY_EXTENSION_ID
                .iter()
                .map(|(extension_id, _)| *extension_id)
                .is_sorted(),
            "suggested extensions must be sorted by id"
        );
    }
}

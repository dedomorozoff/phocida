use anyhow::{Result, anyhow};
use async_trait::async_trait;
use collections::HashMap;
use gpui::AsyncApp;
use language::{LanguageServerName, LspAdapter, LspAdapterDelegate, LspInstaller, Toolchain};
use lsp::LanguageServerBinary;
use project::ContextProviderWithTasks;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use task::{TaskTemplate, TaskTemplates, VariableName};

pub struct PascalLspAdapter;

impl PascalLspAdapter {
    const SERVER_NAME: LanguageServerName = LanguageServerName::new_static("pasls");
}

/// FPC runtime sources and the Lazarus component tree, per distro layout.
/// pasls only guesses `/usr/local/share/{fpcsrc,lazsrc}` (source builds), and
/// without these two it loses the RTL unit paths and returns no completions.
const ENV_FALLBACKS: &[(&str, &[&str])] = &[
    (
        "FPCDIR",
        &[
            "/usr/lib/fpc/src",
            "/usr/share/fpcsrc",
            "/usr/local/share/fpcsrc",
        ],
    ),
    (
        "LAZARUSDIR",
        &[
            "/usr/lib/lazarus",
            "/usr/share/lazarus",
            "/usr/local/share/lazsrc",
        ],
    ),
];

fn detected_env(shell_env: &HashMap<String, String>) -> HashMap<String, String> {
    let mut env = HashMap::default();
    for (key, candidates) in ENV_FALLBACKS {
        if shell_env.contains_key(*key) {
            continue;
        }
        if let Some(dir) = candidates.iter().map(Path::new).find(|dir| dir.is_dir()) {
            env.insert((*key).to_string(), dir.to_string_lossy().into_owned());
        }
    }
    env
}

impl LspInstaller for PascalLspAdapter {
    type BinaryVersion = ();

    async fn check_if_user_installed(
        &self,
        delegate: &Arc<dyn LspAdapterDelegate>,
        _: Option<Toolchain>,
        _: &AsyncApp,
    ) -> Option<LanguageServerBinary> {
        let path = delegate.which(Self::SERVER_NAME.as_ref()).await?;
        Some(LanguageServerBinary {
            path,
            arguments: Vec::new(),
            env: Some(detected_env(&delegate.shell_env().await)),
        })
    }

    async fn fetch_latest_server_version(
        &self,
        _: &Arc<dyn LspAdapterDelegate>,
        _: bool,
        _: &mut AsyncApp,
    ) -> Result<()> {
        Err(pasls_not_installed())
    }

    fn fetch_server_binary(
        &self,
        _latest_version: Self::BinaryVersion,
        _container_dir: PathBuf,
        _: &Arc<dyn LspAdapterDelegate>,
    ) -> impl Send + Future<Output = Result<LanguageServerBinary>> + use<> {
        async move { Err(pasls_not_installed()) }
    }

    async fn cached_server_binary(
        &self,
        _: PathBuf,
        _: &dyn LspAdapterDelegate,
    ) -> Option<LanguageServerBinary> {
        None
    }
}

fn pasls_not_installed() -> anyhow::Error {
    anyhow!(
        "pasls was not found on PATH; install or build pascal-language-server and make sure the `pasls` binary is reachable"
    )
}

#[async_trait(?Send)]
impl LspAdapter for PascalLspAdapter {
    fn name(&self) -> LanguageServerName {
        Self::SERVER_NAME
    }
}

pub(super) fn pascal_task_context() -> ContextProviderWithTasks {
    ContextProviderWithTasks::new(TaskTemplates(vec![TaskTemplate {
        label: format!("build '{}' with fpc", VariableName::File.template_value()),
        command: "fpc".to_owned(),
        args: vec!["-gl".to_owned(), VariableName::File.template_value()],
        tags: vec!["pascal-fpc".to_owned()],
        ..TaskTemplate::default()
    }]))
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, BorrowAppContext, Context, TestAppContext};
    use language::{AutoindentMode, Buffer};
    use settings::SettingsStore;
    use std::num::NonZeroU32;
    use util::test::marked_text_offsets;

    /// Presses enter at `ˇ` (optionally typing `typed` on the new line) and returns the
    /// indent the new line got, in columns.
    fn indent_of_new_line(
        buffer: &mut Buffer,
        cx: &mut Context<Buffer>,
        marked: &str,
        typed: &str,
    ) -> u32 {
        let (source, offsets) = marked_text_offsets(marked);
        buffer.edit([(0..buffer.len(), source.as_str())], None, cx);
        let cursor = offsets[0];
        buffer.edit(
            [(cursor..cursor, format!("\n{typed}"))],
            Some(AutoindentMode::EachLine),
            cx,
        );
        let new_row = source[..cursor].matches('\n').count() as u32 + 1;
        buffer.snapshot().indent_size_for_line(new_row).len
    }

    /// (expected indent, source with the cursor marker, text typed on the new line)
    const CASES: &[(u32, &str, &str)] = &[
        // Declaration sections: open-ended, so siblings keep the section's indent.
        (2, "program Demo;\nvarˇ\nbegin\nend.", ""),
        (2, "program Demo;\nvar\n  A: Integer;ˇ\nbegin\nend.", ""),
        (
            2,
            "program Demo;\nvar\n  A: Integer;ˇ\nbegin\nend.",
            "B: Byte;",
        ),
        (
            0,
            "program Demo;\nvar\n  A: Integer;ˇ\nbegin\nend.",
            "begin",
        ),
        (2, "program Demo;\nconst\n  A = 1;ˇ\nbegin\nend.", "B = 2;"),
        (
            2,
            "unit U;\ninterface\nusesˇ\n  Classes;\nimplementation\nend.",
            "",
        ),
        (
            2,
            "unit U;\ninterface\nuses\n  Classes;ˇ\nimplementation\nend.",
            "SysUtils,",
        ),
        (
            0,
            "unit U;\ninterface\nuses\n  Classes;ˇ\nimplementation\nend.",
            "implementation",
        ),
        (2, "typeˇ\n  TFoo = class\nend;", "TFoo = class"),
        // then/do bodies, and `begin` level with the statement that controls it.
        (2, "program Demo;\nbegin\n  if A > 1 thenˇ\nend.", "begin"),
        (4, "program Demo;\nbegin\n  if A > 1 thenˇ\nend.", ""),
        (
            4,
            "program Demo;\nbegin\n  if A > 1 thenˇ\n    A := 2;\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  if A > 1 then\n    A := 2;ˇ\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  if A > 1 then\n    A := 2;ˇ\nend.",
            "else",
        ),
        (
            2,
            "program Demo;\nbegin\n  if A > 1 then\n    A := 2;\n  elseˇ\nend.",
            "begin",
        ),
        (
            4,
            "program Demo;\nbegin\n  if A > 1 then\n    A := 2;\n  elseˇ\n    A := 3;\nend.",
            "",
        ),
        // `;` before `else` is illegal in Object Pascal, so the then-branch stays unsemicoloned.
        (
            2,
            "program Demo;\nbegin\n  if A > 1 then\n    A := 2\n  else\n    A := 3;ˇ\nend.",
            "",
        ),
        (
            4,
            "program Demo;\nbegin\n  for I := 1 to 3 doˇ\n    A := 2;\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  for I := 1 to 3 do\n    A := 2;ˇ\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  for I := 1 to 3 doˇ\nend.",
            "begin",
        ),
        // Closing keywords return to their opener.
        (0, "program Demo;\nbegin\n  A := 1;ˇ\nend.", "end."),
        (
            4,
            "program Demo;\nbegin\n  repeat\n    A := 1;ˇ\nuntil A > 2;\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  repeat\n    A := 1;ˇ\nuntil A > 2;\nend.",
            "until",
        ),
        (
            4,
            "program Demo;\nbegin\n  try\n    A := 1;ˇ\n  finally\n  end;\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  try\n    A := 1;ˇ\n  finally\n  end;\nend.",
            "except",
        ),
        (
            4,
            "program Demo;\nbegin\n  try\n    A := 1;\n  exceptˇ\n  end;\nend.",
            "on E: Exception do",
        ),
        (
            6,
            "program Demo;\nbegin\n  try\n    A := 1;\n  except\n    on E: Exception doˇ\n      Handle(E);\n  end;\nend.",
            "",
        ),
        (
            2,
            "program Demo;\nbegin\n  case A of\n    1: A := 2;ˇ\n  end;\nend.",
            "else",
        ),
        (
            4,
            "program Demo;\nbegin\n  case A of\n    1: A := 2;ˇ\n  end;\nend.",
            "2: A := 3;",
        ),
        // Type bodies: visibility keywords align with the type name, members one deeper.
        (
            2,
            "type\n  TCalc = class(TObject)ˇ\n  private\n  end;",
            "private",
        ),
        (
            4,
            "type\n  TCalc = class(TObject)\n  privateˇ\n  end;",
            "FSum: Integer;",
        ),
        (
            4,
            "type\n  TCalc = class(TObject)\n  private\n    FSum: Integer;ˇ\n  end;",
            "FOther: Integer;",
        ),
        (
            2,
            "type\n  TCalc = class(TObject)\n  private\n    FSum: Integer;ˇ\n  end;",
            "public",
        ),
        (
            4,
            "type\n  TCalc = class(TObject)\n  private\n    FSum: Integer;\n  publicˇ\n  end;",
            "function Sum: Integer;",
        ),
        (4, "type\n  TPoint = recordˇ\nend;", "X: Integer;"),
        (
            4,
            "type\n  TPoint = record\n    X: Integer;ˇ\nend;",
            "Y: Integer;",
        ),
        (2, "type\n  TPoint = record\n    X: Integer;ˇ\nend;", "end;"),
    ];

    #[gpui::test]
    async fn test_pascal_autoindent(cx: &mut TestAppContext) {
        cx.executor().set_block_on_ticks(usize::MAX..=usize::MAX);
        let language = crate::language("pascal", tree_sitter_pascal::LANGUAGE.into());
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2)
                });
            });
        });

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            let mut mismatches = Vec::new();
            for (expected, marked, typed) in CASES {
                let actual = indent_of_new_line(&mut buffer, cx, marked, typed);
                if actual != *expected {
                    mismatches.push(format!(
                        "expected {expected}, got {actual}: {marked:?} + {typed:?}"
                    ));
                }
            }

            assert!(
                mismatches.is_empty(),
                "{} mismatches:\n{}",
                mismatches.len(),
                mismatches.join("\n")
            );

            buffer
        });
    }
}

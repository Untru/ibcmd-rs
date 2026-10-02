//! What a patch stage cannot take from the tree, said to the user.
//!
//! A patch stage builds each object's rows from the tree and the target's own
//! rows. Some objects it cannot build: one the target does not hold (a new
//! catalog, form or template has no row to patch), predefined data with an
//! item the target's row lacks, an exchange plan whose content names an object
//! the tree does not hold. Each used to end the import with the first such
//! object's technical error in English (`Config row not found: <uuid>`); here
//! every object that fails is collected and named in one refusal, in Russian,
//! with what is unsupported and what to do. Nothing is written: the stage
//! stops before ConfigSave is touched.
//!
//! The errors are recognized by what they say -- the texts of
//! `fetch_config_blob`, the predefined data packer and the exchange plan
//! content packer -- and a test holds each of them, so a changed text fails
//! there.

use std::path::Path;

use super::stage_guard::{StageRefused, display_path};

/// Files listed in a refusal, at most.
const LISTED_OBJECTS: usize = 25;

/// Why a stage cannot build the rows of one object, as far as it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StageProblem {
    /// The target's Config has no row of this name: the object is new to it.
    NoConfigRow { file_name: String },
    /// The tree's predefined data has items (their uuids) the target's row
    /// lacks.
    PredefinedItemsMissing { items: Vec<String> },
    /// A file of the tree names an object (`Catalog.Name`) the tree holds no
    /// file of: `what` is the writer's word for the place (an exchange plan's
    /// content, a role's rights, ...).
    UnresolvedReference { what: String, reference: String },
    /// The build of the object from the tree (`override_stage`) refused it:
    /// the writers' reasons, family by family.
    CannotBuild { reasons: String },
}

/// The places whose writers say `failed to resolve <what> <reference>` when a
/// reference names an object the tree has no file of.
const REFERENCE_PLACES: [&str; 5] = [
    "ExchangePlanContent",
    "Role Rights.xml object",
    "command",
    "child",
    "HTTPService method",
];

/// What `error` -- or the error it wraps -- says, when a stage knows it.
pub(super) fn classify(error: &anyhow::Error) -> Option<StageProblem> {
    const ITEMS_MISSING: &str = "PredefinedData XML contains items missing in base blob: ";
    const ROW_MISSING: &str = "Config row not found: ";
    let texts = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>();
    for text in &texts {
        if let Some(reasons) = text.strip_prefix(super::override_stage::CANNOT_BUILD) {
            return Some(StageProblem::CannotBuild {
                reasons: reasons.to_string(),
            });
        }
        if let Some(differ) = text.strip_prefix(super::override_stage::PREDEFINED_ITEMS_DIFFER) {
            let added = differ
                .strip_prefix("added: ")
                .and_then(|rest| rest.split_once("; removed: "))
                .map(|(added, _)| added)
                .unwrap_or_default();
            return Some(StageProblem::PredefinedItemsMissing {
                items: added
                    .split(", ")
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect(),
            });
        }
        if let Some(items) = text.strip_prefix(ITEMS_MISSING) {
            return Some(StageProblem::PredefinedItemsMissing {
                items: items.split(", ").map(str::to_string).collect(),
            });
        }
        if let Some(rest) = text.strip_prefix("failed to resolve ") {
            for what in REFERENCE_PLACES {
                if let Some(reference) = rest
                    .strip_prefix(what)
                    .and_then(|reference| reference.strip_prefix(' '))
                {
                    return Some(StageProblem::UnresolvedReference {
                        what: what.to_string(),
                        reference: reference.to_string(),
                    });
                }
            }
        }
    }
    texts.iter().find_map(|text| {
        text.strip_prefix(ROW_MISSING)
            .map(|file_name| StageProblem::NoConfigRow {
                file_name: file_name.to_string(),
            })
    })
}

/// One object of the tree the stage could not build.
pub(super) struct ObjectFailure {
    pub xml: std::path::PathBuf,
    pub error: anyhow::Error,
}

/// The refusal for these failures, or the first failure's own error when
/// none of them is one the stage knows how to explain.
pub(super) fn refusal(source_root: &Path, failures: Vec<ObjectFailure>) -> anyhow::Error {
    let known = |failure: &ObjectFailure| classify(&failure.error).is_some();
    if !failures.iter().any(known) {
        return failures
            .into_iter()
            .next()
            .map(|failure| failure.error)
            .unwrap_or_else(|| anyhow::anyhow!("a patch stage failed without an error"));
    }
    let mut lines = vec![format!(
        "Загрузка отменена: в дереве {} есть объекты, которые нельзя загрузить в базу с конфигурацией (всего: {}). В ConfigSave ничего не записано.",
        display_path(source_root),
        failures.len()
    )];
    let mut new_objects = false;
    let mut dangling = false;
    let mut unbuildable = false;
    for failure in failures.iter().take(LISTED_OBJECTS) {
        let path = relative(source_root, &failure.xml);
        match classify(&failure.error) {
            Some(StageProblem::NoConfigRow { file_name }) => {
                new_objects = true;
                lines.push(format!(
                    "  {path}: {} нет в базе (в таблице Config нет строки {file_name}); новые объекты в базу с конфигурацией пока не загружаются",
                    noun(&path)
                ));
            }
            Some(StageProblem::PredefinedItemsMissing { items }) => {
                new_objects = true;
                lines.push(format!(
                    "  {path}: в предопределённых данных есть элементы, которых нет в базе ({}); добавление предопределённых элементов пока не загружается",
                    item_names(&failure.xml, &items)
                ));
            }
            Some(StageProblem::UnresolvedReference { what, reference }) => {
                dangling = true;
                lines.push(format!("  {path}: {}", unresolved_text(&what, &reference)));
            }
            Some(StageProblem::CannotBuild { reasons }) => {
                unbuildable = true;
                lines.push(format!(
                    "  {path}: {} не удалось собрать из дерева: {reasons}",
                    noun_nominative(&path)
                ));
            }
            None => lines.push(format!(
                "  {path}: не удалось собрать строки объекта: {:#}",
                failure.error
            )),
        }
    }
    if failures.len() > LISTED_OBJECTS {
        lines.push(format!(
            "  ... и ещё объектов: {}",
            failures.len() - LISTED_OBJECTS
        ));
    }
    if new_objects {
        lines.push(
            "Что делать: загрузите эту конфигурацию штатным ibcmd (или Конфигуратором). Ключ --base-free собирает все строки из дерева, новые объекты тоже, но в базе с конфигурацией теряет то, чего в XML нет (счётчики схем бизнес-процессов, признак «использовать всегда» у констант): применяйте его только к базе, где это допустимо."
                .to_string(),
        );
    }
    if dangling {
        lines.push(
            "Дерево неполное: на объект ссылаются, а файла нет. --base-free такое дерево тоже не примет.".to_string(),
        );
    }
    if unbuildable {
        lines.push(
            "Причина названа выше: такой объект сборка из дерева не берёт. Исправьте файл дерева или загрузите эту конфигурацию штатным ibcmd (или Конфигуратором)."
                .to_string(),
        );
    }
    anyhow::Error::new(StageRefused::new(lines.join("\n")))
}

/// A reference to an object the tree has no file of, said for the place that
/// holds it.
fn unresolved_text(what: &str, reference: &str) -> String {
    match what {
        "ExchangePlanContent" => format!(
            "состав плана обмена называет {reference}, а файла этого объекта в дереве нет; верните файл в дерево или уберите объект из состава плана обмена"
        ),
        "Role Rights.xml object" => format!(
            "права роли (Ext/Rights.xml) называют {reference}, а файла этого объекта в дереве нет; верните файл в дерево или уберите права на этот объект из роли"
        ),
        _ => format!(
            "ссылка на {reference} ({what}) не находит объект в дереве; верните файл объекта или уберите ссылку"
        ),
    }
}

/// The path of an object's file relative to the tree, `/`-separated.
fn relative(root: &Path, path: &Path) -> String {
    let stripped = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| {
            // One of the two is a `\\?\` path and the other is not.
            let verbatim = |value: &Path| {
                value
                    .to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .to_string()
            };
            let (root, path) = (verbatim(root), verbatim(path));
            // A `\\?\` path is a Windows spelling: its separators are
            // backslashes wherever this runs.
            match path.strip_prefix(&root) {
                Some(rest) => rest
                    .split(['\\', '/'])
                    .filter(|part| !part.is_empty())
                    .collect(),
                None => std::path::PathBuf::from(path),
            }
        });
    stripped
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// `объект`, `форму`, `макет` ...: what the file at `path` is, as the object of
/// "could not build".
fn noun_nominative(path: &str) -> &'static str {
    match noun(path) {
        "формы" => "форму",
        "макета" => "макет",
        "справочника" => "справочник",
        "документа" => "документ",
        "перечисления" => "перечисление",
        "роли" => "роль",
        "подсистемы" => "подсистему",
        "общего модуля" => "общий модуль",
        "отчёта" => "отчёт",
        "обработки" => "обработку",
        "регистра сведений" => "регистр сведений",
        "регистра накопления" => "регистр накопления",
        "константы" => "константу",
        "плана обмена" => "план обмена",
        _ => "объект",
    }
}

/// `объекта`, `формы`, `макета` ...: what the file at `path` is, in the
/// genitive.
fn noun(path: &str) -> &'static str {
    if path.contains("/Forms/") || path.starts_with("CommonForms/") {
        return "формы";
    }
    if path.contains("/Templates/") || path.starts_with("CommonTemplates/") {
        return "макета";
    }
    match path.split('/').next().unwrap_or_default() {
        "Catalogs" => "справочника",
        "Documents" => "документа",
        "Enums" => "перечисления",
        "Roles" => "роли",
        "Subsystems" => "подсистемы",
        "CommonModules" => "общего модуля",
        "Reports" => "отчёта",
        "DataProcessors" => "обработки",
        "InformationRegisters" => "регистра сведений",
        "AccumulationRegisters" => "регистра накопления",
        "Constants" => "константы",
        "ExchangePlans" => "плана обмена",
        _ => "объекта",
    }
}

/// The names of predefined items, read from the tree's file by their uuids;
/// the uuids themselves when the file cannot be read.
fn item_names(xml: &Path, ids: &[String]) -> String {
    let file = super::infer_predefined_data_body_path(xml);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    ids.iter()
        .map(|id| predefined_item_name(&text, id).unwrap_or_else(|| id.clone()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The `<Name>` of the `<Item id="...">` in a predefined data file.
fn predefined_item_name(text: &str, id: &str) -> Option<String> {
    let at = text.find(&format!("id=\"{id}\""))?;
    let rest = &text[at..];
    let start = rest.find("<Name>")? + "<Name>".len();
    let end = rest[start..].find("</Name>")?;
    Some(rest[start..start + end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn failure(root: &Path, relative: &str, error: anyhow::Error) -> ObjectFailure {
        ObjectFailure {
            xml: relative
                .split('/')
                .fold(root.to_path_buf(), |path, part| path.join(part)),
            error,
        }
    }

    fn message(error: &anyhow::Error) -> String {
        error
            .downcast_ref::<StageRefused>()
            .unwrap_or_else(|| panic!("not a refusal: {error:#}"))
            .to_string()
    }

    fn no_row(file_name: &str) -> anyhow::Error {
        anyhow::anyhow!("Config row not found: {file_name}")
    }

    /// The errors of the stage as they reach the refusal: the texts the
    /// stage's own functions produce, with their contexts.
    #[test]
    fn the_stages_errors_are_recognized_by_their_texts() {
        assert_eq!(
            classify(&no_row("6e8a3462-4c7f-4883-8bb1-e1e3035412fb")),
            Some(StageProblem::NoConfigRow {
                file_name: "6e8a3462-4c7f-4883-8bb1-e1e3035412fb".to_string()
            })
        );
        let predefined = anyhow::anyhow!(
            "PredefinedData XML contains items missing in base blob: c8c4ba53, 0c13a8e0"
        )
        .context("failed to pack PredefinedData C:\\tree\\Catalogs\\A\\Ext\\Predefined.xml");
        assert_eq!(
            classify(&predefined),
            Some(StageProblem::PredefinedItemsMissing {
                items: vec!["c8c4ba53".to_string(), "0c13a8e0".to_string()]
            })
        );
        let content = anyhow::anyhow!("failed to read Catalog XML x: not found")
            .context("failed to resolve ExchangePlanContent Catalog.Old")
            .context("failed to pack ExchangePlan Content y");
        assert_eq!(
            classify(&content),
            Some(StageProblem::UnresolvedReference {
                what: "ExchangePlanContent".to_string(),
                reference: "Catalog.Old".to_string()
            })
        );
        let rights = anyhow::anyhow!("failed to read Catalog XML x: not found")
            .context("failed to resolve Role Rights.xml object Catalog.Old")
            .context("failed to pack Role rights y");
        assert_eq!(
            classify(&rights),
            Some(StageProblem::UnresolvedReference {
                what: "Role Rights.xml object".to_string(),
                reference: "Catalog.Old".to_string()
            })
        );
        // The writers' other "failed to resolve" contexts are not references.
        assert_eq!(
            classify(&anyhow::anyhow!("failed to resolve TypeId from C:\\x")),
            None
        );
        assert_eq!(classify(&anyhow::anyhow!("something else")), None);
    }

    #[test]
    fn new_objects_are_named_with_their_kind_and_what_to_do() {
        let root = PathBuf::from("C:\\tree");
        let text = message(&refusal(
            &root,
            vec![
                failure(
                    &root,
                    "Catalogs/ДемоНовыйСправочник.xml",
                    no_row("6e8a3462-4c7f-4883-8bb1-e1e3035412fb"),
                ),
                failure(
                    &root,
                    "Catalogs/Заметки/Forms/ДемоНоваяФорма.xml",
                    no_row("ac0f02f9-59c8-4a03-a023-60a840ca5468"),
                ),
            ],
        ));
        let lines = text.lines().collect::<Vec<_>>();
        assert!(
            lines[0].starts_with("Загрузка отменена: в дереве C:\\tree есть объекты"),
            "{text}"
        );
        assert!(
            lines[0].contains("нельзя загрузить в базу с конфигурацией (всего: 2)"),
            "{text}"
        );
        assert!(
            lines[0].ends_with("В ConfigSave ничего не записано."),
            "{text}"
        );
        assert!(
            lines[1].starts_with("  Catalogs/ДемоНовыйСправочник.xml: справочника нет в базе"),
            "{text}"
        );
        assert!(
            lines[2].starts_with("  Catalogs/Заметки/Forms/ДемоНоваяФорма.xml: формы нет в базе"),
            "{text}"
        );
        assert!(text.contains("штатным ibcmd"), "{text}");
        assert!(text.contains("--base-free"), "{text}");
        assert!(!text.contains("Config row not found"), "{text}");
    }

    #[test]
    fn a_dangling_exchange_plan_content_is_not_sent_to_base_free() {
        let root = PathBuf::from("C:\\tree");
        let error = anyhow::anyhow!("failed to read Catalog XML x: not found")
            .context("failed to resolve ExchangePlanContent Catalog.Удалить_Старый")
            .context("failed to pack ExchangePlan Content y");
        let text = message(&refusal(
            &root,
            vec![failure(&root, "ExchangePlans/Обмен.xml", error)],
        ));
        assert!(
            text.contains(
                "  ExchangePlans/Обмен.xml: состав плана обмена называет Catalog.Удалить_Старый"
            ),
            "{text}"
        );
        assert!(
            text.contains("уберите объект из состава плана обмена"),
            "{text}"
        );
        assert!(!text.contains("Что делать: загрузите"), "{text}");
        assert!(
            text.contains("--base-free такое дерево тоже не примет"),
            "{text}"
        );
    }

    #[test]
    fn rights_naming_a_removed_object_are_named_too() {
        let root = PathBuf::from("C:\\tree");
        let error = anyhow::anyhow!("failed to read Catalog XML x: not found")
            .context("failed to resolve Role Rights.xml object Catalog.Удалить_Старый")
            .context("failed to pack Role rights y");
        let text = message(&refusal(
            &root,
            vec![failure(&root, "Roles/Администратор.xml", error)],
        ));
        assert!(
            text.contains("  Roles/Администратор.xml: права роли (Ext/Rights.xml) называют Catalog.Удалить_Старый"),
            "{text}"
        );
        assert!(
            text.contains("уберите права на этот объект из роли"),
            "{text}"
        );
        assert!(!text.contains("failed to"), "{text}");
    }

    #[test]
    fn an_object_the_build_could_not_make_is_named_with_the_reason() {
        let root = PathBuf::from("C:\tree");
        let error = anyhow::anyhow!(
            "{}kind body: the writer refuses it",
            super::super::override_stage::CANNOT_BUILD
        );
        let text = message(&refusal(
            &root,
            vec![failure(&root, "Catalogs/A.xml", error)],
        ));
        assert!(
            text.contains("  Catalogs/A.xml: справочник не удалось собрать из дерева: kind body: the writer refuses it"),
            "{text}"
        );
        assert!(text.contains("Исправьте файл дерева"), "{text}");
    }

    #[test]
    fn predefined_items_are_named_by_the_tree() {
        let root =
            std::env::temp_dir().join(format!("ibcmd-rs-patch-refusal-{}", std::process::id()));
        let folder = root.join("Catalogs").join("Состояния").join("Ext");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("Predefined.xml"),
            "<PredefinedData><Item id=\"c8c4ba53-68cf-45d3-98d2-2b4b1a9c7b0a\">\n<Name>ДемоНовыйЭлемент</Name>\n</Item></PredefinedData>",
        )
        .unwrap();
        let text = message(&refusal(
            &root,
            vec![failure(
                &root,
                "Catalogs/Состояния.xml",
                anyhow::anyhow!(
                    "PredefinedData XML contains items missing in base blob: c8c4ba53-68cf-45d3-98d2-2b4b1a9c7b0a"
                )
                .context("failed to pack PredefinedData x"),
            )],
        ));
        assert!(text.contains("(ДемоНовыйЭлемент)"), "{text}");
        assert!(
            text.contains("добавление предопределённых элементов пока не загружается"),
            "{text}"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn errors_of_unknown_kind_pass_through_untouched() {
        let root = PathBuf::from("C:\\tree");
        let error = refusal(
            &root,
            vec![failure(
                &root,
                "Catalogs/A.xml",
                anyhow::anyhow!("something else broke"),
            )],
        );
        assert_eq!(error.to_string(), "something else broke");
        assert!(error.downcast_ref::<StageRefused>().is_none());
    }

    #[test]
    fn an_unknown_failure_among_known_ones_is_listed_too() {
        let root = PathBuf::from("C:\\tree");
        let text = message(&refusal(
            &root,
            vec![
                failure(&root, "Catalogs/New.xml", no_row("x")),
                failure(
                    &root,
                    "Catalogs/Odd.xml",
                    anyhow::anyhow!("something else broke"),
                ),
            ],
        ));
        assert!(
            text.contains(
                "  Catalogs/Odd.xml: не удалось собрать строки объекта: something else broke"
            ),
            "{text}"
        );
    }

    #[test]
    fn paths_are_shown_relative_to_the_tree_even_when_one_side_is_verbatim() {
        assert_eq!(
            relative(
                Path::new(r"\\?\F:\tree"),
                Path::new(r"F:\tree\Catalogs\A.xml")
            ),
            "Catalogs/A.xml"
        );
        assert_eq!(
            relative(Path::new(r"F:\tree"), Path::new(r"F:\tree\Catalogs\A.xml")),
            "Catalogs/A.xml"
        );
    }

    #[test]
    fn a_long_list_is_cut() {
        let root = PathBuf::from("C:\\tree");
        let failures = (0..LISTED_OBJECTS + 4)
            .map(|index| {
                failure(
                    &root,
                    &format!("Catalogs/C{index:03}.xml"),
                    no_row(&format!("row{index}")),
                )
            })
            .collect();
        let text = message(&refusal(&root, failures));
        assert!(text.contains("  Catalogs/C024.xml"), "{text}");
        assert!(!text.contains("Catalogs/C025.xml"), "{text}");
        assert!(text.contains("... и ещё объектов: 4"), "{text}");
    }
}

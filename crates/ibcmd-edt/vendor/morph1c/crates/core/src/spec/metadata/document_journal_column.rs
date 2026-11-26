//! Канонический спек ДОЧЕРНЕГО вида `DocumentJournal.Column` (графа журнала документов) —
//! child-objects substrate, DocumentJournal-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! ПОЛНЫЙ симметричный sub-object (uuid+name+Properties в обоих форматах). Графа журнала
//! проецирует одноимённые реквизиты нескольких зарегистрированных документов: несёт
//! `references` — список ссылок вида `Document.<Doc>.Attribute.<Attr>` /
//! `Document.<Doc>.StandardAttribute.<Std>` (ref-list item-style, как `registeredDocuments`).
//!
//! Поля (сверено 2/2 ОБА формата): Synonym, Comment, Indexing, References. Порядок —
//! Designer DENSE `<Properties>`. EDT эмитит РАЗРЕЖЁННО (synonym всегда present; comment
//! Designer-only; indexing омитится при дефолте DontIndex; references — multi-sibling).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "" (Designer-only DENSE `<Comment/>`; EDT не несёт).
pub const F_COMMENT: FieldId = FieldId(2);
/// `indexing` — индексирование графы (enum). Default DontIndex.
pub const F_INDEXING: FieldId = FieldId(3);
/// `references` — список ссылок на реквизиты документов (item-style). Default [].
pub const F_REFERENCES: FieldId = FieldId(4);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_INDEXING, "indexing", ValueKind::Enum, PropertyValue::Enum(Token::new("DontIndex"))),
        FieldSpec::with_default(F_REFERENCES, "references", ValueKind::List, PropertyValue::List(Vec::new())),
    ]
}

/// `&'static EntitySpec` вида `DocumentJournal.Column` (кэш на процесс).
pub fn document_journal_column() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DocumentJournal.Column",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

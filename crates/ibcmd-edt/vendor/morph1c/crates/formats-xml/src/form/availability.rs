//! Native SDK unavailable-path spelling for EDT XML 2.20/2.21 projections.
//! Query selection grammar is adapted from ibcmd's existing
//! src/mssql_dump/form_body.rs; query bytes and canonical paths are unchanged.
use super::FormError;
use morph1c_core::{
    ir::{DynamicListAttrExt, FormBody},
    version::{FormatVersion, current_roundtrip_target},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
struct Availability {
    default_picture_unavailable: bool,
    fields: Option<BTreeSet<String>>,
}
thread_local! { static AVAILABILITY: RefCell<BTreeMap<String,Availability>> = const {RefCell::new(BTreeMap::new())}; }
pub(crate) fn unavailable(path: &str) -> bool {
    let Some((owner, field)) = path.split_once('.') else {
        return false;
    };
    AVAILABILITY.with(|map| {
        let map = map.borrow();
        let Some(a) = map.get(owner) else {
            return false;
        };
        if field == "DefaultPicture" {
            return a.default_picture_unavailable;
        }
        if field.contains('.') {
            return false;
        }
        a.fields.as_ref().is_some_and(|fields| {
            !fields
                .iter()
                .any(|value| value.to_lowercase() == field.to_lowercase())
        })
    })
}
pub(crate) fn with_availability<T>(
    body: &FormBody,
    f: impl FnOnce() -> Result<T, FormError>,
) -> Result<T, FormError> {
    struct Restore(BTreeMap<String, Availability>);
    impl Drop for Restore {
        fn drop(&mut self) {
            AVAILABILITY.with(|m| *m.borrow_mut() = std::mem::take(&mut self.0));
        }
    }
    let mut map = BTreeMap::new();
    if !body.designer_path_spelling
        && matches!(current_roundtrip_target(), Some(FormatVersion { major: 2, minor: 20 | 21 }))
    {
        for attr in &body.data_attributes {
            if let Some(list) = &attr.dynamic_list {
                let no_picture = list.main_table.as_deref().is_none_or(|table| {
                    table
                        .split_once('.')
                        .is_some_and(|(family, _)| matches!(family, "Enum" | "FilterCriterion"))
                });
                let fields = fields(list)?;
                if map
                    .insert(
                        attr.name.clone(),
                        Availability {
                            default_picture_unavailable: no_picture,
                            fields,
                        },
                    )
                    .is_some()
                {
                    return Err(FormError::Frame(
                        "duplicate dynamic-list attribute identity".into(),
                    ));
                }
            }
        }
    }
    let _restore = Restore(AVAILABILITY.with(|m| m.replace(map)));
    f()
}
fn fields(list: &DynamicListAttrExt) -> Result<Option<BTreeSet<String>>, FormError> {
    if !list.custom_query || !list.auto_fill_available_fields {
        return Ok(None);
    }
    let Some(query) = &list.query_text else {
        return Ok(None);
    };
    let Some(mut fields) = selection(query)? else {
        return Ok(None);
    };
    // Standard platform columns also have localized field-map twins. Do not
    // interpret unrelated property bags as paths or inspect query string contents.
    for kind in [
        "Catalog",
        "Document",
        "Enum",
        "ChartOfCharacteristicTypes",
        "ChartOfAccounts",
        "ChartOfCalculationTypes",
        "ExchangePlan",
        "BusinessProcess",
        "Task",
        "InformationRegister",
        "AccumulationRegister",
        "AccountingRegister",
        "CalculationRegister",
        "DocumentJournal",
    ] {
        if let Some(pairs) = standard_pairs(kind) {
            for (ru, en) in pairs {
                if fields.contains(*ru) {
                    fields.insert((*en).into());
                }
            }
        }
    }
    if let Some(table) = &list.main_table {
        let Some(pairs) = standard_pairs(table.split('.').next().unwrap_or_default()) else {
            return Ok(None);
        };
        for (ru, en) in pairs {
            fields.insert((*ru).into());
            fields.insert((*en).into());
        }
    }
    for calculated in &list.calculated_fields {
        fields.insert(calculated.data_path.clone());
    }
    Ok(Some(fields))
}
fn ident(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && c.all(|c| c.is_alphanumeric() || c == '_')
}
fn keyword(s: &str, words: &[&str]) -> bool {
    words.iter().any(|w| s.to_uppercase() == *w)
}
fn tokens(query: &str) -> Result<Vec<String>, FormError> {
    const BYTES: usize = 4 * 1024 * 1024;
    const TOKENS: usize = 262144;
    if query.len() > BYTES {
        return Err(FormError::Frame(
            "dynamic-list query exceeds bounded projection byte limit".into(),
        ));
    }
    let chars: Vec<char> = query.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut stack = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        if c == '"' {
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Err(FormError::Frame("unterminated query string".into()));
            }
        } else if c.is_alphanumeric() || c == '_' || c == '&' {
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
        } else {
            i += 1;
            if matches!(c, '(' | '{') {
                stack.push(c);
                if stack.len() > 64 {
                    return Err(FormError::Frame("query projection depth limit".into()));
                }
            } else if matches!(c, ')' | '}')
                && stack.pop() != Some(if c == ')' { '(' } else { '{' })
            {
                return Err(FormError::Frame("unbalanced query delimiter".into()));
            }
        }
        out.push(chars[start..i].iter().collect());
        if out.len() > TOKENS {
            return Err(FormError::Frame("query projection token limit".into()));
        }
    }
    if !stack.is_empty() {
        return Err(FormError::Frame("unbalanced query delimiter".into()));
    }
    Ok(out)
}
fn top_keyword(tokens: &[String], words: &[&str]) -> bool {
    let mut depth = 0;
    tokens.iter().any(|t| {
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        depth == 0 && keyword(t, words)
    })
}
fn selection(query: &str) -> Result<Option<BTreeSet<String>>, FormError> {
    let tokens = tokens(query)?;
    let mut batches = Vec::new();
    let mut begin = 0;
    let mut depth = 0;
    for (i, t) in tokens.iter().enumerate() {
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 && t == ";" {
            batches.push(&tokens[begin..i]);
            begin = i + 1;
        }
    }
    batches.push(&tokens[begin..]);
    let Some(batch) = batches.into_iter().rfind(|b| {
        top_keyword(b, &["ВЫБРАТЬ", "SELECT"]) && !top_keyword(b, &["ПОМЕСТИТЬ", "INTO"])
    }) else {
        return Ok(None);
    };
    let Some(mut i) = batch
        .iter()
        .position(|t| keyword(t, &["ВЫБРАТЬ", "SELECT"]))
    else {
        return Ok(None);
    };
    i += 1;
    while i < batch.len()
        && (keyword(
            &batch[i],
            &[
                "РАЗРЕШЕННЫЕ",
                "ALLOWED",
                "РАЗЛИЧНЫЕ",
                "DISTINCT",
                "ПЕРВЫЕ",
                "TOP",
            ],
        ) || batch[i].chars().all(|c| c.is_ascii_digit()))
    {
        i += 1;
    }
    let mut terms = Vec::new();
    let mut term = Vec::new();
    depth = 0;
    for t in &batch[i..] {
        if depth == 0
            && keyword(
                t,
                &[
                    "ИЗ",
                    "FROM",
                    "ГДЕ",
                    "WHERE",
                    "ПОМЕСТИТЬ",
                    "INTO",
                    "СГРУППИРОВАТЬ",
                    "GROUP",
                    "УПОРЯДОЧИТЬ",
                    "ORDER",
                    "ОБЪЕДИНИТЬ",
                    "UNION",
                    "ИТОГИ",
                    "TOTALS",
                ],
            )
        {
            break;
        }
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 && t == "," {
            terms.push(std::mem::take(&mut term));
        } else {
            term.push(t.as_str());
        }
    }
    if !term.is_empty() {
        terms.push(term);
    }
    if terms.is_empty() {
        return Ok(None);
    }
    let mut fields = BTreeSet::new();
    for term in terms {
        let n = term.len();
        if n == 0 || term.contains(&"*") || term.contains(&"{") {
            return Ok(None);
        }
        let name = if n >= 2 && keyword(term[n - 2], &["КАК", "AS"]) && ident(term[n - 1]) {
            term[n - 1].to_owned()
        } else if n % 2 == 1
            && term
                .iter()
                .enumerate()
                .all(|(i, t)| if i % 2 == 0 { ident(t) } else { *t == "." })
        {
            let start = if n >= 5 && keyword(term[2], &["ССЫЛКА", "REF"]) {
                4
            } else if n > 1 {
                2
            } else {
                0
            };
            term[start..].iter().step_by(2).copied().collect()
        } else if n == 1 && term[0].starts_with('&') && ident(&term[0][1..]) {
            term[0][1..].to_owned()
        } else {
            // Expressions without an explicit alias do not prove a complete
            // result-field universe. In particular, the trailing member of
            // A.X + B.Y is not an implicit alias.
            return Ok(None);
        };
        fields.insert(name);
    }
    Ok(Some(fields))
}

fn standard_pairs(kind: &str) -> Option<&'static [(&'static str, &'static str)]> {
    const CATALOG: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("Родитель", "Parent"),
        ("ЭтоГруппа", "IsFolder"),
        ("Владелец", "Owner"),
    ];
    const DOCUMENT: [(&str, &str); 7] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Проведен", "Posted"),
        ("ВерсияДанных", "DataVersion"),
        ("МоментВремени", "PointInTime"),
    ];
    const ENUM: [(&str, &str); 2] = [("Ссылка", "Ref"), ("Порядок", "Order")];
    const CHART_OF_CHARACTERISTIC_TYPES: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("ТипЗначения", "ValueType"),
        ("Родитель", "Parent"),
        ("ЭтоГруппа", "IsFolder"),
    ];
    const CHART_OF_ACCOUNTS: [(&str, &str); 11] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("Родитель", "Parent"),
        ("Порядок", "Order"),
        ("Забалансовый", "OffBalance"),
        ("Вид", "Type"),
    ];
    const CHART_OF_CALCULATION_TYPES: [(&str, &str); 8] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("ПериодДействияБазовый", "ActionPeriodIsBasic"),
    ];
    const EXCHANGE_PLAN: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("НомерОтправленного", "SentNo"),
        ("НомерПринятого", "ReceivedNo"),
        ("ЭтотУзел", "ThisNode"),
    ];
    const BUSINESS_PROCESS: [(&str, &str); 8] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("ВерсияДанных", "DataVersion"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Завершен", "Completed"),
        ("Стартован", "Started"),
        ("ВедущаяЗадача", "HeadTask"),
    ];
    const TASK: [(&str, &str); 9] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("ВерсияДанных", "DataVersion"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Наименование", "Description"),
        ("БизнесПроцесс", "BusinessProcess"),
        ("ТочкаМаршрута", "RoutePoint"),
        ("Выполнена", "Executed"),
    ];
    const INFORMATION_REGISTER: [(&str, &str); 4] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
    ];
    const ACCUMULATION_REGISTER: [(&str, &str); 5] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
        ("ВидДвижения", "RecordType"),
    ];
    const CALCULATION_REGISTER: [(&str, &str); 12] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
        ("ПериодРегистрации", "RegistrationPeriod"),
        ("ПериодДействия", "ActionPeriod"),
        ("ПериодДействияНачало", "BegOfActionPeriod"),
        ("ПериодДействияКонец", "EndOfActionPeriod"),
        ("БазовыйПериодНачало", "BegOfBasePeriod"),
        ("БазовыйПериодКонец", "EndOfBasePeriod"),
        ("ВидРасчета", "CalculationType"),
        ("Сторно", "ReversingEntry"),
    ];
    const DOCUMENT_JOURNAL: [(&str, &str); 6] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Проведен", "Posted"),
        ("Тип", "Type"),
    ];
    match kind {
        "Catalog" => Some(&CATALOG),
        "Document" => Some(&DOCUMENT),
        "Enum" => Some(&ENUM),
        "ChartOfCharacteristicTypes" => Some(&CHART_OF_CHARACTERISTIC_TYPES),
        "ChartOfAccounts" => Some(&CHART_OF_ACCOUNTS),
        "ChartOfCalculationTypes" => Some(&CHART_OF_CALCULATION_TYPES),
        "ExchangePlan" => Some(&EXCHANGE_PLAN),
        "BusinessProcess" => Some(&BUSINESS_PROCESS),
        "Task" => Some(&TASK),
        "InformationRegister" => Some(&INFORMATION_REGISTER),
        "AccumulationRegister" => Some(&ACCUMULATION_REGISTER),
        "AccountingRegister" => Some(&INFORMATION_REGISTER),
        "CalculationRegister" => Some(&CALCULATION_REGISTER),
        "DocumentJournal" => Some(&DOCUMENT_JOURNAL),
        _ => None,
    }
}

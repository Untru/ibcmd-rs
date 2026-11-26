use super::*;

// M1: эмиттер экранирует РОВНО `& < >` (конвенция 1С); кавычки/иное — литералы.
#[test]
fn escaping_minimal_set_amp_lt_gt() {
    assert_eq!(
        escape_text("a & b < c > d \" e"),
        "a &amp; b &lt; c &gt; d \" e"
    );
    // Кавычки в тексте не экранируются (1С их оставляет литеральными).
    assert_eq!(escape_text("\"q\""), "\"q\"");
    // Атрибут — та же тройка.
    assert_eq!(escape_attr("a&b<c>d"), "a&amp;b&lt;c&gt;d");
}

// M1-симметрия: parse(strict_unescape) ∘ escape == identity на `& < >`.
#[test]
fn parse_unescapes_what_emit_escapes() {
    let env = Envelope {
        bom: false,
        eol: "\n",
        indent_unit: "  ",
        decl: "<?xml version=\"1.0\"?>",
        trailing_eol: true,
        escape_gt: true,
        escape_quot: false,
        text_eol: "
",
    };
    let mut root = OutElement::branch("", "root").attr("a", "x&y<z>w");
    root.push(OutElement::leaf("", "leaf", "p & q < r > s"));
    let bytes = emit::render(&env, &root);
    let parsed = parse(&bytes).expect("parse");
    assert_eq!(parsed.root.attr("a").unwrap().value, "x&y<z>w");
    assert_eq!(parsed.root.child("leaf").unwrap().text, "p & q < r > s");
}

// M1: эмиттированный escape переживает re-parse byte-exact (render∘parse∘render).
#[test]
fn escape_roundtrips_byte_exact() {
    let env = Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "  ",
        decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        trailing_eol: true,
        escape_gt: true,
        escape_quot: false,
        text_eol: "
",
    };
    let mut root = OutElement::branch("", "root");
    root.push(OutElement::leaf("", "leaf", "a < b & c > d"));
    let first = emit::render(&env, &root);
    let parsed = parse(&first).expect("parse");
    // Пере-эмитим из распарсенного значения → те же байты.
    let mut root2 = OutElement::branch("", "root");
    root2.push(OutElement::leaf(
        "",
        "leaf",
        parsed.root.child("leaf").unwrap().text.clone(),
    ));
    let second = emit::render(&env, &root2);
    assert_eq!(first, second);
}

// M1: числовая/нерепродуцируемая сущность → типизированная ошибка, не lossy.
#[test]
fn non_reproducible_entities_error() {
    assert!(matches!(
        parse("<root><leaf>a&#10;b</leaf></root>".as_bytes()),
        Err(XmlReadError::NonReproducibleEntity(_))
    ));
    // `&quot;` в АТРИБУТЕ нерепродуцируемо (эмиттер пишет атрибут с литеральной
    // кавычкой) → ошибка. В ТЕКСТЕ — допустимо (EDT-конвенция, см. ниже).
    assert!(matches!(
        parse("<root a=\"x&quot;y\"></root>".as_bytes()),
        Err(XmlReadError::NonReproducibleEntity(_))
    ));
    // `&apos;` нерепродуцируема в любом контексте (эмиттер апостроф пишет литерально).
    assert!(matches!(
        parse("<root><leaf>a&apos;b</leaf></root>".as_bytes()),
        Err(XmlReadError::NonReproducibleEntity(_))
    ));
    assert!(matches!(
        parse("<root a=\"&#xA;\"></root>".as_bytes()),
        Err(XmlReadError::NonReproducibleEntity(_))
    ));
    // Допустимые `&amp;/&lt;/&gt;` — НЕ ошибка.
    assert!(parse("<root><leaf>a &amp; b &lt; c</leaf></root>".as_bytes()).is_ok());
    // `&quot;` в ТЕКСТЕ — допустимо (EDT экранирует `"`→`&quot;`; эмиттер per-format
    // восстанавливает) → парсится, текст = `a"b`.
    let q = parse("<root><leaf>a&quot;b</leaf></root>".as_bytes()).expect("quot in text ok");
    assert_eq!(q.root.child("leaf").unwrap().text, "a\"b");
}

// Неподдерживаемые конструкции (CDATA/комментарий/DOCTYPE) → типизированная
// ошибка, не silent-skip (§1.0).
#[test]
fn unsupported_constructs_error() {
    assert!(matches!(
        parse(b"<root><![CDATA[x]]></root>"),
        Err(XmlReadError::Unsupported(_))
    ));
    assert!(matches!(
        parse(b"<root><!-- c --></root>"),
        Err(XmlReadError::Unsupported(_))
    ));
}

// B1 (субстрат) — непустой невостребованный текст считается несконсуменным узлом.
#[test]
fn b1_unclaimed_text_counts() {
    let parsed = parse(b"<root>STRAY<child>x</child></root>").expect("parse");
    // Ничего не claim'им: корень-узел + его текст STRAY + child-узел + child-текст
    // = 4 невостребованных. Ключевое: STRAY (текст корня) попадает в счёт.
    assert_eq!(parsed.root.unclaimed_count(), 4);
    // Claim'им корень-узел (но НЕ текст): STRAY всё ещё несконсуменный.
    parsed.root.claim();
    assert_eq!(parsed.root.unclaimed_count(), 3);
    // Claim'им текст корня: STRAY учтён.
    parsed.root.claim_text();
    assert_eq!(parsed.root.unclaimed_count(), 2); // остались child-узел + текст
}

// B2 (субстрат) — префикс — часть идентичности; child() по local-name + проверка
// префикса в decode. Проверяем, что распарсенный префикс сохранён.
#[test]
fn b2_prefix_preserved_on_parse() {
    let parsed = parse(b"<root><ns:leaf>v</ns:leaf></root>").expect("parse");
    let leaf = parsed.root.child("leaf").expect("found by local-name");
    assert_eq!(leaf.prefix, "ns");
    assert_eq!(leaf.local, "leaf");
}

// M2 (субстрат) — наблюдение BOM/EOL из сырых байт.
#[test]
fn m2_envelope_observation() {
    let crlf = parse(b"<root>\r\n</root>").expect("parse");
    assert!(!crlf.bytes_env.bom);
    assert_eq!(crlf.bytes_env.eol, EolStyle::Crlf);

    let lf = parse(b"<root>\n</root>").expect("parse");
    assert_eq!(lf.bytes_env.eol, EolStyle::Lf);

    let mut bom_bytes = vec![0xEF, 0xBB, 0xBF];
    bom_bytes.extend_from_slice(b"<root></root>");
    let bom = parse(&bom_bytes).expect("parse");
    assert!(bom.bytes_env.bom);
    assert_eq!(bom.bytes_env.eol, EolStyle::None);
}

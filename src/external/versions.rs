//! The `versions` service entry (`{1,N,"",<ver>,"<entry>",<ver>,…}`): a version
//! id per container entry. The pipeline checks it against the entry list, so
//! it follows the adapter's renames, drops and additions.

use anyhow::{Context, Result, bail};

use super::brace;

pub fn rewrite(
    text: &str,
    rename: &[(&str, &str)],
    drop: &[&str],
    add: &[(&str, &str)],
) -> Result<String> {
    let (f, _) =
        brace::fields(brace::strip_bom(text).trim(), 0).context("versions is not a brace list")?;
    if f.first() != Some(&"1") || f.len() < 2 || (f.len() - 2) % 2 != 0 {
        bail!("unsupported versions layout");
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in f[2..].chunks(2) {
        let name = pair[0].trim_matches('"');
        if drop.contains(&name) {
            continue;
        }
        let name = rename
            .iter()
            .find(|(from, _)| from.eq_ignore_ascii_case(name))
            .map_or(name, |(_, to)| to);
        pairs.push((name.to_owned(), pair[1].to_owned()));
    }
    pairs.extend(add.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())));
    let body: Vec<String> = pairs.iter().map(|(n, v)| format!("\"{n}\",{v}")).collect();
    Ok(format!("\u{feff}{{1,{},{}}}", pairs.len(), body.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "\u{feff}{1,4,\"\",00000000-0000-0000-0000-00000000000a,\"main\",00000000-0000-0000-0000-00000000000b,\"copyinfo\",00000000-0000-0000-0000-00000000000c,\"root\",00000000-0000-0000-0000-00000000000d}";

    #[test]
    fn renames_drops_and_adds_entries_and_recounts() {
        let out = rewrite(
            V,
            &[("main", "obj")],
            &["copyinfo"],
            &[("types", "00000000-0000-0000-0000-00000000000e")],
        )
        .unwrap();
        assert_eq!(
            out,
            "\u{feff}{1,4,\"\",00000000-0000-0000-0000-00000000000a,\"obj\",00000000-0000-0000-0000-00000000000b,\"root\",00000000-0000-0000-0000-00000000000d,\"types\",00000000-0000-0000-0000-00000000000e}"
        );
    }

    #[test]
    fn odd_pair_list_fails_closed() {
        assert!(rewrite("{1,1,\"\"}", &[], &[], &[]).is_err());
    }
}

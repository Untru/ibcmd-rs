// Research assertions for the existing public API; not an independent ICU oracle.
use ibcmd_filedb::index::{Collation, MAX_KEY_NAME, config_key};

fn main() {
    let mut checks = 0usize;
    macro_rules! check {
        ($name:expr, $condition:expr) => {{
            assert!($condition, "{}", $name);
            checks += 1;
            println!("PASS\t{}", $name);
        }};
    }

    check!("documented_max_name_43", MAX_KEY_NAME == 43);
    for collation in [Collation::Posix, Collation::Russian] {
        for length in [0, 1, 43, 44] {
            let key = config_key(collation, &"x".repeat(length), 0);
            let expected = matches!(length, 1 | 43);
            check!(
                format!("{} length {length}", collation.locale()),
                key.is_ok() == expected && key.as_ref().map_or(true, |key| key.len() == 392)
            );
        }
        let admitted = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-./_";
        check!(
            format!("{} admitted characters", collation.locale()),
            admitted.chars().all(|character| {
                config_key(collation, &character.to_string(), 0).is_ok_and(|key| key.len() == 392)
            })
        );
        for name in [" ", "а", "é", "\0"] {
            check!(
                format!("{} refuses {name:?}", collation.locale()),
                config_key(collation, name, 0).is_err()
            );
        }
        check!(
            format!("{} distinct parts", collation.locale()),
            config_key(collation, "root", 0).unwrap() != config_key(collation, "root", 1).unwrap()
        );
        check!(
            format!("{} maximum u32 part", collation.locale()),
            config_key(collation, "root", u32::MAX).is_ok_and(|key| key.len() == 392)
        );
    }
    check!(
        "Russian case folding",
        config_key(Collation::Russian, "DBNames", 0).unwrap()
            == config_key(Collation::Russian, "dbnames", 0).unwrap()
    );
    check!(
        "POSIX case distinction",
        config_key(Collation::Posix, "DBNames", 0).unwrap()
            != config_key(Collation::Posix, "dbnames", 0).unwrap()
    );
    check!(
        "unmeasured locale refused",
        Collation::for_locale("de_DE").is_none()
    );
    println!("checks={checks}; failures=0; independent_icu_comparison=false");
}

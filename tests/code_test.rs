use std::{fs, io};

use nospace20::{interpret_func_testing, parse_to_tokens, parse_to_tree, syntactic_analyze};

fn test_ok_coding_base(test_name: &str) {
    let path_base = "resources/test/".to_owned() + test_name;
    let ns_cnt = fs::read_to_string(path_base.to_owned() + ".ns")
        .expect("Something went wrong reading the file");

    let t = parse_to_tokens(&ns_cnt).ok().unwrap();
    let s = parse_to_tree(&t).ok().unwrap();
    let a = syntactic_analyze(&s);
    let trace = interpret_func_testing(&a, "main");
    let check_json: serde_json::Value = serde_json::from_reader(io::BufReader::new(
        fs::File::open(path_base.to_owned() + ".check.json")
            .ok()
            .unwrap(),
    ))
    .ok()
    .unwrap();
    let expected_trace = check_json
        .get("trace")
        .unwrap()
        .as_array()
        .unwrap()
        .into_iter()
        .map(|e| e.as_i64().unwrap());
    for (i, expected) in expected_trace.enumerate() {
        let key = i as i64;
        if let Some(actual) = trace.get(&key) {
            assert_eq!(expected, *actual, "trace failed");
        } else {
            panic!("idx:{} trace doesn't exist", key);
        }
    }
}

macro_rules! test_ok_coding {
    ($name: ident, $test_name: expr) => {
        // TODO: concat_idents! is only for nightly
        #[test]
        fn $name() -> Result<(), &'static str> {
            test_ok_coding_base($test_name);
            Ok(())
        }
    };
}

test_ok_coding!(test_ok_coding_c000, "c000");

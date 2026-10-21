use std::collections::BTreeMap;

/// `trace_hit_counts` の期待値と、実行で記録された `__trace` のヒット回数を照合する
///
/// 期待値の添字 `i` をキーとし、`traced[i]` が期待値と一致することを確かめる。
/// キーが記録されていなければ失敗する。期待値にない（添字の範囲外の）キーは照合しない。
/// `label` は失敗時のメッセージに含める（テスト名・ターゲット名など）
pub fn assert_trace_hit_counts(expected: &[i64], traced: &BTreeMap<i64, i64>, label: &str) {
    for (i, expected) in expected.iter().enumerate() {
        let key = i as i64;
        match traced.get(&key) {
            Some(actual) => assert_eq!(
                *expected, *actual,
                "trace(idx:{}) failed ({}). traced: {:?}",
                key, label, traced
            ),
            None => panic!(
                "idx:{} trace doesn't exist ({}). traced: {:?}",
                key, label, traced
            ),
        }
    }
}

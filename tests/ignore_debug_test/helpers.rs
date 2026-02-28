//! ignore_debug テスト用のヘルパー関数

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use nospace20::{
    parse_to_tokens, parse_to_tree, syntactic_analyze, Environment, EnvironmentConfig, Scope,
};

/// EnvironmentConfig を使って interpret_func を実行するヘルパー
pub fn interpret_func_with_config(
    scope: &Scope,
    func_name: &str,
    config: EnvironmentConfig,
) -> (std::collections::BTreeMap<i64, i64>, String) {
    use std::io::Write;

    let stdin_cursor = Box::new(std::io::BufReader::new(Cursor::new(Vec::<u8>::new())));

    let stdout_buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let stdout_clone = Rc::clone(&stdout_buf);

    struct SharedWriter(Rc<RefCell<Vec<u8>>>);
    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.borrow_mut().flush()
        }
    }

    let stdout_writer: Box<dyn Write> = Box::new(SharedWriter(stdout_clone));
    let mut env = Environment::new_with_config(stdin_cursor, stdout_writer, config);

    nospace20::interpret_func_with_env(&mut env, scope, func_name).unwrap();

    let stdout_vec = stdout_buf.borrow().clone();
    let stdout_string = String::from_utf8(stdout_vec).unwrap();

    (env.traced, stdout_string)
}

pub fn parse_and_analyze(source: &str) -> Scope {
    let tokens = parse_to_tokens(&source.to_string()).unwrap();
    let tree = parse_to_tree(&tokens).unwrap();
    #[allow(deprecated)]
    syntactic_analyze(&tree).unwrap()
}

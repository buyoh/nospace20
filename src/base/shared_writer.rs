use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// `Rc<RefCell<Vec<u8>>>` をラップして `Write` トレイトを実装する
///
/// stdout バッファを複数箇所で共有するために使用する。
pub struct SharedWriter(pub Rc<RefCell<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

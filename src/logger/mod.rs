pub struct TextCode<'a> {
    // source: &'a str,
    lines: Vec<&'a str>,
    line_indices: Vec<usize>,
}

// TODO: add a function that convert from char based index to byte based index.
impl<'a> TextCode<'a> {
    pub fn new(source: &'a str) -> Self {
        // NOTE: do not use lines().
        // lines は '\n' で区切った後、末尾の '\r' の削除を試みる。
        // 削除してしまうと、 char の個数の予想が難しくなる。
        let lines = source.split_terminator('\n').collect();
        let mut sum = 0;
        let line_indices = (&lines as &Vec<&'a str>)
            .iter()
            .map(|li| {
                let t = sum;
                sum += li.chars().count() + 1;
                t
            })
            .collect();
        // TODO: indexize
        Self {
            // source,
            lines,
            line_indices,
        }
    }

    pub fn line(&self, i: usize) -> &'a str {
        let line = self.lines[i];
        let l = line.len();
        if l > 0 && line.as_bytes()[l - 1] == b'\r' {
            &line[0..l - 1]
        } else {
            line
        }
    }

    // (line, column)
    pub fn char_index_to_line(&self, i: usize) -> (usize, usize) {
        match self.line_indices.binary_search(&i) {
            Ok(li) => (li, 0),
            Err(li) => (li - 1, i - self.line_indices[li - 1]),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_char_index_to_line_single_line() {
        let code = TextCode::new("abc");
        assert_eq!(code.char_index_to_line(0), (0, 0));
        assert_eq!(code.char_index_to_line(1), (0, 1));
        assert_eq!(code.char_index_to_line(2), (0, 2));
    }

    #[test]
    fn test_char_index_to_line_multi_line() {
        let code = TextCode::new("abc\ndef\nghi");
        assert_eq!(code.char_index_to_line(0), (0, 0));
        assert_eq!(code.char_index_to_line(3), (0, 3));
        assert_eq!(code.char_index_to_line(4), (1, 0));
        assert_eq!(code.char_index_to_line(7), (1, 3));
        assert_eq!(code.char_index_to_line(8), (2, 0));
    }

    #[test]
    fn test_line_method() {
        let code = TextCode::new("abc\ndef");
        assert_eq!(code.line(0), "abc");
        assert_eq!(code.line(1), "def");
    }

    #[test]
    fn test_line_with_carriage_return() {
        let code = TextCode::new("abc\r\ndef\r\n");
        assert_eq!(code.line(0), "abc");
        assert_eq!(code.line(1), "def");
    }
}

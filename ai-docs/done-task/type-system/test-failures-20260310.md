# Test failures (2026-03-10)

## Summary
- test_syntax_error_invalid_token_001 now fails because `@` is a valid token.

## Details
- Test: test_syntax_error_invalid_token_001
- File: resources/tests/fails/syntax/invalid_token_001.ns
- Expected: tokenize error (parse_error phase tokenize)
- Actual: tokenization succeeds because `@` is now a valid token for type annotations.
- Likely cause: Phase 1 adds the `@` token, so this test no longer represents an invalid token case.

## Next steps
- Replace the test input with a truly invalid token, or update the expectation to a parse error.
- Leave the existing test unchanged for now per task instructions.

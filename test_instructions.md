# Testing Indentation in Lektor

## How to test:

1. Run the editor:
   ```
   ./target/release/lektor test_indent.py
   ```

2. Test indentation:
   - Move cursor to any line
   - Press `Tab` to indent the current line
   - Press `Shift+Tab` to unindent the current line

3. Test multi-line indentation:
   - Select multiple lines using `Shift+Down` or `Shift+Up`
   - Press `Tab` to indent all selected lines
   - Press `Shift+Tab` to unindent all selected lines

4. Test in Insert mode:
   - Press `i` to enter Insert mode
   - Select text with `Shift+Arrow` keys
   - Use `Tab` and `Shift+Tab` to indent/unindent

## Notes:
- Shift+Tab is recognized as `BackTab` by the terminal
- The indentation respects your config settings (spaces vs tabs)
- Selection is maintained after indenting/unindenting

//! Shell escaping utilities to prevent command injection
//!
//! When passing commands to SSH or WSL, the remote shell interprets
//! metacharacters. This module provides safe escaping to prevent injection.

/// Escapes a string for safe use in a POSIX shell command.
///
/// Uses single quotes which prevent all interpretation except for single quotes
/// themselves. Single quotes within the string are escaped by ending the quoted
/// section, adding an escaped single quote, and starting a new quoted section.
///
/// # Example
/// ```ignore
/// let safe = shell_escape("hello; rm -rf /");
/// assert_eq!(safe, "'hello; rm -rf /'");
///
/// let with_quote = shell_escape("it's");
/// assert_eq!(with_quote, "'it'\\''s'");
/// ```
pub fn shell_escape(s: &str) -> String {
    // If the string is empty, return empty quoted string
    if s.is_empty() {
        return "''".to_string();
    }

    // Check if string contains only safe characters (alphanumeric, dash, underscore, dot, slash)
    // If so, no escaping needed
    let is_safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@' | '='));

    if is_safe {
        return s.to_string();
    }

    // Use single quotes, escaping any embedded single quotes
    // 'it'\''s' -> the shell sees: it's
    let mut result = String::with_capacity(s.len() + 10);
    result.push('\'');

    for c in s.chars() {
        if c == '\'' {
            // End current quote, add escaped quote, start new quote
            result.push_str("'\\''");
        } else {
            result.push(c);
        }
    }

    result.push('\'');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_strings_unchanged() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("file.txt"), "file.txt");
        assert_eq!(shell_escape("/path/to/file"), "/path/to/file");
        assert_eq!(shell_escape("user@host"), "user@host");
        assert_eq!(shell_escape("key=value"), "key=value");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_spaces_quoted() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn test_single_quotes_escaped() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_dangerous_metacharacters() {
        // Command substitution
        assert_eq!(shell_escape("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(shell_escape("`rm -rf /`"), "'`rm -rf /`'");

        // Command chaining
        assert_eq!(shell_escape("foo; rm -rf /"), "'foo; rm -rf /'");
        assert_eq!(shell_escape("foo && rm -rf /"), "'foo && rm -rf /'");
        assert_eq!(shell_escape("foo || rm -rf /"), "'foo || rm -rf /'");

        // Pipes
        assert_eq!(shell_escape("foo | rm -rf /"), "'foo | rm -rf /'");

        // Redirections
        assert_eq!(shell_escape("foo > /etc/passwd"), "'foo > /etc/passwd'");
        assert_eq!(shell_escape("foo < /etc/passwd"), "'foo < /etc/passwd'");

        // Glob patterns
        assert_eq!(shell_escape("rm -rf *"), "'rm -rf *'");
        assert_eq!(shell_escape("file?.txt"), "'file?.txt'");

        // Variable expansion
        assert_eq!(shell_escape("$HOME"), "'$HOME'");
        assert_eq!(shell_escape("${PATH}"), "'${PATH}'");

        // Newlines (command injection via newline)
        assert_eq!(shell_escape("foo\nrm -rf /"), "'foo\nrm -rf /'");
    }

    #[test]
    fn test_complex_injection_attempt() {
        // Real-world injection attempt
        let malicious = "'; cat /etc/passwd; echo '";
        let escaped = shell_escape(malicious);
        // The string starts with ', so we get: '' (empty) + \' (escaped quote) + '; cat...' + \' + '' (empty)
        assert_eq!(escaped, "''\\''; cat /etc/passwd; echo '\\'''");
    }
}

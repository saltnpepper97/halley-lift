use std::path::Path;

/// Desktop Entry string escapes are decoded before Exec quoting and expansion.
pub fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(ch) => {
                // Preserve quoting escapes for the second decoding step. This
                // also accepts the single-escaped form used by older entries.
                out.push('\\');
                out.push(ch);
            }
            None => out.push('\\'),
        }
    }
    out
}

struct Argument {
    text: String,
    quoted: bool,
}

fn arguments(value: &str) -> Result<Vec<Argument>, String> {
    let mut chars = value.chars().peekable();
    let mut args = Vec::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_whitespace() {
            chars.next();
            continue;
        }
        let quoted = ch == '"';
        let mut text = String::new();
        if quoted {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some(ch @ ('"' | '`' | '$' | '\\')) => text.push(ch),
                        _ => return Err("Invalid escape in quoted desktop Exec argument".into()),
                    },
                    Some(ch) => text.push(ch),
                    None => return Err("Unclosed quote in desktop Exec".into()),
                }
            }
            if chars.peek().is_some_and(|ch| !ch.is_ascii_whitespace()) {
                return Err("Desktop Exec arguments must be quoted in whole".into());
            }
        } else {
            while let Some(ch) = chars.peek().copied() {
                if ch.is_ascii_whitespace() {
                    break;
                }
                if matches!(
                    ch,
                    '"' | '\''
                        | '\\'
                        | '>'
                        | '<'
                        | '~'
                        | '|'
                        | '&'
                        | ';'
                        | '$'
                        | '*'
                        | '?'
                        | '#'
                        | '('
                        | ')'
                        | '`'
                ) {
                    return Err("Reserved character in unquoted desktop Exec argument".into());
                }
                text.push(ch);
                chars.next();
            }
        }
        args.push(Argument { text, quoted });
    }
    Ok(args)
}

/// Expand supported field codes once, retaining every argument boundary. Lift
/// has no file/URL selection, so file fields and deprecated fields are omitted.
pub fn parse(
    value: &str,
    name: &str,
    icon: Option<&str>,
    path: &Path,
) -> Result<Vec<String>, String> {
    let args = arguments(&unescape(value))?;
    let mut result = Vec::new();
    let mut file_fields = 0;
    for (index, arg) in args.into_iter().enumerate() {
        if arg.text == "%i" && !arg.quoted {
            if index == 0 {
                return Err("Desktop Exec must start with an executable".into());
            }
            if let Some(icon) = icon.filter(|value| !value.is_empty()) {
                result.extend(["--icon".into(), icon.into()]);
            }
            continue;
        }
        let mut expanded = String::new();
        let mut chars = arg.text.chars();
        let mut removed_field = false;
        while let Some(ch) = chars.next() {
            if ch != '%' {
                expanded.push(ch);
                continue;
            }
            let code = chars
                .next()
                .ok_or("Incomplete field code in desktop Exec")?;
            if code == '%' {
                expanded.push('%');
                continue;
            }
            if index == 0 || arg.quoted {
                return Err("Field code in desktop Exec executable or quoted argument".into());
            }
            match code {
                'c' => expanded.push_str(name),
                'k' => expanded.push_str(&path.to_string_lossy()),
                'f' | 'u' | 'F' | 'U' => {
                    if matches!(code, 'F' | 'U') && arg.text != format!("%{code}") {
                        return Err("Desktop Exec list field must be a separate argument".into());
                    }
                    file_fields += 1;
                    if file_fields > 1 {
                        return Err("Multiple file/URL fields in desktop Exec".into());
                    }
                    removed_field = true;
                }
                'd' | 'D' | 'n' | 'N' | 'v' | 'm' => removed_field = true,
                'i' => return Err("Desktop Exec icon field must be a separate argument".into()),
                _ => return Err(format!("Unknown desktop Exec field code %{code}")),
            }
        }
        if !expanded.is_empty() || !removed_field {
            result.push(expanded);
        }
    }
    if result
        .first()
        .is_none_or(|program| program.is_empty() || program.contains('='))
    {
        return Err("Desktop Exec has no valid executable".into());
    }
    if result.iter().any(|arg| arg.contains('\0')) {
        return Err("NUL in desktop Exec argument".into());
    }
    Ok(result)
}

/// The public cluster API accepts shell command text, so quote each already
/// expanded argument when crossing that boundary. Never re-expand field values.
pub fn shell_command(args: &[String]) -> String {
    args.iter()
        .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(value: &str) -> Result<Vec<String>, String> {
        parse(
            value,
            "Éditor's $HOME %k",
            Some("test icon"),
            Path::new("/apps/test file.desktop"),
        )
    }

    #[test]
    fn quoted_paths_spaces_and_empty_arguments_survive() {
        assert_eq!(
            command(r#""/apps/test program" "two  spaces" "" plain"#).unwrap(),
            ["/apps/test program", "two  spaces", "", "plain"]
        );
    }

    #[test]
    fn string_escapes_are_decoded_once_before_quote_escapes() {
        let raw = r#"app "line\nnext\tcolumn\rend\sspace" "literal \\\\n" "slash \\\\" "quote \\"" "\\$HOME" "\\`cmd\\`""#;
        assert_eq!(
            command(raw).unwrap(),
            [
                "app",
                "line\nnext\tcolumn\rend space",
                r"literal \n",
                r"slash \",
                "quote \"",
                "$HOME",
                "`cmd`"
            ]
        );
        assert_eq!(unescape(r"one\\ntwo\nthree"), "one\\ntwo\nthree");
    }

    #[test]
    fn fields_expand_once_without_splitting_or_evaluating_values() {
        assert_eq!(
            command("app %c --name=%c %k %i %% percent%%tail").unwrap(),
            [
                "app",
                "Éditor's $HOME %k",
                "--name=Éditor's $HOME %k",
                "/apps/test file.desktop",
                "--icon",
                "test icon",
                "%",
                "percent%tail"
            ]
        );
        assert_eq!(
            parse("app %i", "App", None, Path::new("test.desktop")).unwrap(),
            ["app"]
        );
        assert_eq!(
            parse("app %i", "App", Some(""), Path::new("test.desktop")).unwrap(),
            ["app"]
        );
    }

    #[test]
    fn unused_file_fields_and_deprecated_fields_are_removed() {
        for code in ['f', 'F', 'u', 'U'] {
            assert_eq!(
                command(&format!("app %{code} %d %D %n %N %v %m end")).unwrap(),
                ["app", "end"]
            );
        }
        assert_eq!(
            command("app --file=%f end").unwrap(),
            ["app", "--file=", "end"]
        );
    }

    #[test]
    fn invalid_commands_are_rejected_instead_of_partially_launched() {
        for value in [
            "",
            "\"\"",
            "VAR=value app",
            "app %x",
            "app %",
            "app %f %u",
            "app --files=%F",
            "app --icon=%i",
            "app \"%c\"",
            "app \"unclosed",
            "app partial\"quote\"",
            "app \"closed\"suffix",
            "app 'single quoted'",
            "app unquoted;command",
            "app \"bad\\q\"",
            "app \"nul\0\"",
            "%f app",
        ] {
            assert!(command(value).is_err(), "accepted {value:?}");
        }
    }
}

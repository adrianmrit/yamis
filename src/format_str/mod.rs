use crate::types::DynErrResult;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "format_str/grammar.pest"]
struct StrFormatParser;

/// Renames the rules for better error messages
fn rename_rules(rule: &Rule) -> String {
    match rule {
        Rule::escaped_val => "%%".to_string(),
        Rule::literal_content => "literal".to_string(),
        Rule::literal => "literal".to_string(),
        Rule::format_param => "%s".to_string(),
        Rule::EOI => "EOI".to_string(),
        __other__ => panic!("Unexpected rule {:?}", __other__),
    }
}

/// Formats the given string with positional parameters. Values in the format string
/// matching `{}` will be replaced by the corresponding values. Brackets can be escaped
/// by having two of them in a row, i.e. `{{`.
///
/// # Arguments
///
/// * `fmt_string`: String to replace the values at
/// * `vars`: Values to replace for
///
/// returns: Result<String, Box<dyn Error, Global>>
pub fn format_string<S: AsRef<str>>(fmt_string: S, vars: &[&str]) -> DynErrResult<String> {
    let tokens = StrFormatParser::parse(Rule::all, fmt_string.as_ref());

    let tokens = match tokens {
        Ok(mut tokens) => tokens.next().unwrap().into_inner(),
        Err(e) => {
            return Err(format!(
                "Invalid format string:\n{}",
                e.renamed_rules(rename_rules).to_string()
            )
            .into())
        }
    };

    let mut result = String::new();
    let mut i = 0;
    for token in tokens {
        match token.as_rule() {
            Rule::literal => {
                for literal in token.into_inner() {
                    match literal.as_rule() {
                        Rule::escaped_val => result.push('%'),
                        Rule::literal_content => result.push_str(literal.as_str()),
                        _ => {
                            panic!("Unexpected token {}", literal.as_str());
                        }
                    }
                }
            }
            Rule::format_param => match vars.get(i) {
                None => {
                    return Err("Not enough variables".into());
                }
                Some(val) => {
                    result.push_str(val.as_ref());
                    i += 1;
                }
            },
            Rule::EOI => {
                break;
            }
            _ => {
                panic!("Unexpected token {}", token.as_str());
            }
        }
    }
    Ok(result)
}

#[test]
fn test_format_string() {
    let fmt_string = "Hello %s %s %s %%s";
    let vars = vec!["world", "!", "?"];
    let result = format_string(fmt_string, &vars).unwrap();
    assert_eq!(result, "Hello world ! ? %s");

    let fmt_string = "";
    let vars = vec!["world", "!", "?"];
    let result = format_string(fmt_string, &vars).unwrap();
    assert_eq!(result, "");

    let fmt_string = " ";
    let vars = vec!["world", "!", "?"];
    let result = format_string(fmt_string, &vars).unwrap();
    assert_eq!(result, " ");

    let fmt_string = " %%";
    let vars = vec!["world", "!", "?"];
    let result = format_string(fmt_string, &vars).unwrap();
    assert_eq!(result, " %");

    let fmt_string = " %";
    let vars = vec!["world", "!", "?"];
    let result = format_string(fmt_string, &vars).unwrap_err().to_string();
    let expected_result = r#"Invalid format string:
 --> 1:2
  |
1 |  %
  |  ^---
  |
  = expected EOI, literal, or %s"#;
    assert_eq!(result, expected_result);
}

use colored::{Color, ColoredString, Colorize};

const PREFIX: &str = "[YAMIS]";

pub trait YamisOutput {
    /// Returns the given string with the `[YAMIS]` prefix in each line. The prefix will also take the given color.
    fn yamis_prefix<S: Into<Color>>(&self, color: S) -> String;
}

impl YamisOutput for str {
    fn yamis_prefix<S: Into<Color>>(&self, color: S) -> String {
        let lines = self.split_inclusive('\n');
        let prefix = PREFIX.color(color);

        let mut result = String::new();
        for line in lines {
            result.push_str(&format!("{} {}", prefix, line));
        }
        result
    }
}

// Calling the function in a ColoredString instance removes the color from it,
// so we need to transform it to a string first to keep it.
impl YamisOutput for ColoredString {
    fn yamis_prefix<S: Into<Color>>(&self, color: S) -> String {
        self.to_string().yamis_prefix(color)
    }
}

#[test]
fn test_yamis_prefix() {
    let prefix = PREFIX.color(Color::Red);
    let output = "\nThis is a test\n\nThis is another test\n\n".to_string();
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!(
        "{prefix} \n{prefix} This is a test\n{prefix} \n{prefix} This is another test\n{prefix} \n"
    );
    assert_eq!(colored_output, expected_output);

    let output = "This is a test\nThis is another test";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!("{prefix} This is a test\n{prefix} This is another test");
    assert_eq!(colored_output, expected_output);

    let colored_text = "This is a test".color(Color::Red);
    let output = format!("{colored_text}\nThis is another test");
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!("{prefix} {colored_text}\n{prefix} This is another test");
    assert_eq!(colored_output, expected_output);

    let output = "This is a test\n";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!("{prefix} This is a test\n");
    assert_eq!(colored_output, expected_output);

    let output = "This is a test";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!("{prefix} This is a test");
    assert_eq!(colored_output, expected_output);

    let output = "\n\n";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = format!("{prefix} \n{prefix} \n");
    assert_eq!(colored_output, expected_output);

    let output = "";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = "";
    assert_eq!(colored_output, expected_output);
}

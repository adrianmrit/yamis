use colored::{Color, ColoredString, Colorize};

const PREFIX: &str = "[YAMIS]";
const INFO_COLOR: Color = Color::BrightBlue;
const WARN_COLOR: Color = Color::BrightYellow;
const ERROR_COLOR: Color = Color::BrightRed;

pub trait YamisOutput {
    /// Returns the given string with the `[YAMIS]` prefix in each line. The prefix will also take the given color.
    fn yamis_prefix<S: Into<Color> + Clone>(&self, color: S) -> String;
    /// Adds the `[YAMIS]` prefix to the given string. The whole string will have the given color.
    fn yamis_colorize<S: Into<Color> + Clone>(&self, color: S) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The whole string will be blue.
    fn yamis_info(&self) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The prefix will be blue.
    fn yamis_prefix_info(&self) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The whole string will be yellow.
    fn yamis_warn(&self) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The prefix will be yellow.
    fn yamis_prefix_warn(&self) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The whole string will be red.
    fn yamis_error(&self) -> String;
    /// Returns the given string with the `[YAMIS]` prefix in each line. The prefix will be red.
    fn yamis_prefix_error(&self) -> String;
}

impl YamisOutput for str {
    fn yamis_prefix<S: Into<Color> + Clone>(&self, color: S) -> String {
        let lines = self.split_inclusive('\n');
        let prefix = PREFIX.color(color).to_string();

        let mut result = String::new();
        for line in lines {
            result.push_str(&prefix);
            result.push(' ');
            result.push_str(line);
        }
        result
    }

    fn yamis_colorize<S: Into<Color> + Clone>(&self, color: S) -> String {
        let lines = self.split_inclusive('\n');

        let mut result = String::new();
        for line in lines {
            result.push_str(PREFIX);
            result.push(' ');
            result.push_str(line);
        }
        result.color(color).to_string()
    }

    fn yamis_info(&self) -> String {
        self.yamis_colorize(INFO_COLOR)
    }

    fn yamis_prefix_info(&self) -> String {
        self.yamis_prefix(INFO_COLOR)
    }

    fn yamis_warn(&self) -> String {
        self.yamis_colorize(WARN_COLOR)
    }

    fn yamis_prefix_warn(&self) -> String {
        self.yamis_prefix(WARN_COLOR)
    }

    fn yamis_error(&self) -> String {
        self.yamis_colorize(ERROR_COLOR)
    }

    fn yamis_prefix_error(&self) -> String {
        self.yamis_prefix(ERROR_COLOR)
    }
}

// Calling the function in a ColoredString instance removes the color from it,
// so we need to transform it to a string first to keep it.
impl YamisOutput for ColoredString {
    fn yamis_prefix<S: Into<Color> + Clone>(&self, color: S) -> String {
        self.to_string().yamis_prefix(color)
    }

    fn yamis_colorize<S: Into<Color> + Clone>(&self, color: S) -> String {
        self.to_string().yamis_colorize(color)
    }

    fn yamis_info(&self) -> String {
        self.to_string().yamis_info()
    }

    fn yamis_prefix_info(&self) -> String {
        self.to_string().yamis_prefix_info()
    }

    fn yamis_warn(&self) -> String {
        self.to_string().yamis_warn()
    }

    fn yamis_prefix_warn(&self) -> String {
        self.to_string().yamis_prefix_warn()
    }

    fn yamis_error(&self) -> String {
        self.to_string().yamis_error()
    }

    fn yamis_prefix_error(&self) -> String {
        self.to_string().yamis_prefix_error()
    }
}

#[test]
fn test_yamis_prefix() {
    let info_prefix = PREFIX.color(INFO_COLOR);
    let warn_prefix = PREFIX.color(WARN_COLOR);
    let error_prefix = PREFIX.color(ERROR_COLOR);

    let output = "\nThis is a test\n\nThis is another test\n\n".to_string();
    let colored_output = output.yamis_prefix_error();
    let expected_output = format!(
        "{error_prefix} \n{error_prefix} This is a test\n{error_prefix} \n{error_prefix} This is another test\n{error_prefix} \n"
    );
    assert_eq!(colored_output, expected_output);

    let output = "This is a test\nThis is another test";
    let colored_output = output.yamis_prefix_error();
    let expected_output =
        format!("{error_prefix} This is a test\n{error_prefix} This is another test");
    assert_eq!(colored_output, expected_output);

    let output = "This is a test\nThis is another test";
    let colored_output = output.yamis_error();
    let expected_output = format!("{PREFIX} This is a test\n{PREFIX} This is another test")
        .color(ERROR_COLOR)
        .to_string();
    assert_eq!(colored_output, expected_output);

    let colored_text = "This is a test".color(Color::Blue);
    let output = format!("{colored_text}\nThis is another test");
    let colored_output = output.yamis_prefix_warn();
    let expected_output =
        format!("{warn_prefix} {colored_text}\n{warn_prefix} This is another test");
    assert_eq!(colored_output, expected_output);

    let output = "This is a test\n";
    let colored_output = output.yamis_prefix_info();
    let expected_output = format!("{info_prefix} This is a test\n");
    assert_eq!(colored_output, expected_output);

    let output = "This is a test";
    let colored_output = output.yamis_info();
    let expected_output = format!("{PREFIX} This is a test")
        .color(INFO_COLOR)
        .to_string();
    assert_eq!(colored_output, expected_output);

    let output = "\n\n";
    let colored_output = output.yamis_prefix_info();
    let expected_output = format!("{info_prefix} \n{info_prefix} \n");
    assert_eq!(colored_output, expected_output);

    let output = "";
    let colored_output = output.yamis_prefix(Color::Red);
    let expected_output = "";
    assert_eq!(colored_output, expected_output);
}

use std::{cmp, fs, io, path::Path, str::FromStr};

use crossterm::{
    ExecutableCommand, event, execute,
    style::{
        Attribute::{Bold, Dim, NoBold, Reset},
        Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor,
    },
};
use syntect::{
    easy::HighlightLines, highlighting::Theme, parsing::SyntaxSet, util::as_24_bit_terminal_escaped,
};
use vscode_theme_syntect::VscodeTheme;

use crate::{Diagnostic, Label, LabelStyle, Severity, SourceSpan};

#[derive(Debug)]
pub struct CliRenderer {
    ps: SyntaxSet,
    dark_theme: Theme,
    light_theme: Theme,
}

impl Default for CliRenderer {
    fn default() -> Self {
        let ps = SyntaxSet::load_defaults_newlines();
        let dark_theme = Theme::try_from(
            VscodeTheme::from_str(include_str!("./theme/dark.json"))
                .expect("failed to load dark theme"),
        )
        .expect("failed to convert dark theme");
        let light_theme = Theme::try_from(
            VscodeTheme::from_str(include_str!("./theme/light.json"))
                .expect("failed to load light theme"),
        )
        .expect("failed to convert light theme");

        Self {
            ps,
            dark_theme,
            light_theme,
        }
    }
}

const FG_GRAY: SetForegroundColor = SetForegroundColor(Color::DarkGrey);
// const FG_RED: SetForegroundColor = SetForegroundColor(Color::Red);
// const FG_YELLOW: SetForegroundColor = SetForegroundColor(Color::Yellow);
// const FG_BLUE: SetForegroundColor = SetForegroundColor(Color::Blue);
const FG_GREEN: SetForegroundColor = SetForegroundColor(Color::Green);
const FG_CYAN: SetForegroundColor = SetForegroundColor(Color::Cyan);

impl CliRenderer {
    #[must_use]
    pub fn render(&self, diagnostic: &Diagnostic) -> String {
        let mut out = String::new();
        self.render_header(diagnostic, &mut out);

        if let Some(primary) = diagnostic.primary_label() {
            self.render_primary_location(primary, &mut out);
        }

        for label in &diagnostic.labels {
            self.render_label(label, diagnostic.severity, &mut out);
        }

        for note in &diagnostic.notes {
            out.push_str(&format!(
                "   {FG_GRAY}╧ {FG_CYAN}{Bold}note:{Reset} {note}\n"
            ));
        }
        for help in &diagnostic.help {
            out.push_str(&format!(
                "   {FG_GRAY}╧ {FG_GREEN}{Bold}help:{Reset} {help}\n"
            ));
        }

        out.trim_end().to_string()
    }

    fn render_header(&self, diagnostic: &Diagnostic, out: &mut String) {
        let severity = self.paint_severity(diagnostic.severity);
        if let Some(code) = &diagnostic.code {
            out.push_str(&format!(
                "{severity}[{code}]: {Bold}{}{Reset}\n",
                diagnostic.message
            ));
        } else {
            out.push_str(&format!(
                "{severity}: {Bold}{}{Reset}\n",
                diagnostic.message
            ));
        }
    }

    fn render_primary_location(&self, label: &Label, out: &mut String) {
        if let (Some(file), Some(span)) = (&label.file, label.span) {
            out.push_str(&format!(
                "   {FG_GRAY}╭─[{FG_CYAN}{Dim}{}:{}:{}{Reset}{FG_GRAY}]{ResetColor}\n",
                file.display(),
                span.start.line,
                span.start.column
            ));
        }
    }

    fn render_label(&self, label: &Label, severity: Severity, out: &mut String) {
        let (Some(file), Some(span)) = (&label.file, label.span) else {
            if let Some(message) = &label.message {
                out.push_str(&format!(
                    "  ╧ {}: {message}\n",
                    label_style_name(label.style)
                ));
            }

            return;
        };

        let line_no = span.start.line;
        let line_no_width = line_no.to_string().len();

        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(err) => {
                self.render_unavailable_source(file, err, label, out, line_no_width);
                return;
            }
        };

        self.render_source_span(file, &source, span, label, severity, out);
    }

    fn render_unavailable_source(
        &self,
        file: &Path,
        err: io::Error,
        label: &Label,
        out: &mut String,
        line_no_width: usize,
    ) {
        out.push_str(&format!(
            "{0:>line_no_width$} {FG_GRAY}│{ResetColor}\n{0:>line_no_width$} ╧ {Bold}note:{Reset} could not read {1}: {err}\n",
            "",
            file.display(),
        ));
        if let Some(message) = &label.message {
            out.push_str(&format!(
                "{:>line_no_width$} ╧ {}: {message}\n",
                "",
                label_style_name(label.style)
            ));
        }
    }

    fn render_source_span(
        &self,
        file: &Path,
        source: &str,
        span: SourceSpan,
        label: &Label,
        severity: Severity,
        out: &mut String,
    ) {
        let lines = source.lines().collect::<Vec<_>>();
        let Some(line) = lines.get(span.start.line.saturating_sub(1)) else {
            return;
        };

        let line_no = span.start.line;
        let line_no_width = line_no.to_string().len();

        let syntax = self
            .ps
            .find_syntax_for_file(file)
            .expect("syntax not found for file")
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &self.dark_theme);

        let ranges = h
            .highlight_line(line, &self.ps)
            .expect("failed to highlight line");
        let line = as_24_bit_terminal_escaped(&ranges, false);

        out.push_str(&format!("{:>line_no_width$} {FG_GRAY}│\n", ""));
        out.push_str(&format!(
            "{line_no:>line_no_width$} {FG_GRAY}│{ResetColor} {line}{ResetColor}\n"
        ));

        let start_col = span.start.column.saturating_sub(1);
        let end_col = if span.end.line == span.start.line {
            span.end.column.saturating_sub(1)
        } else {
            line.chars().count()
        };
        let width = cmp::max(1, end_col.saturating_sub(start_col));
        let marker = match label.style {
            LabelStyle::Primary => "^",
            LabelStyle::Secondary => "-",
        };

        let color = match severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Note => Color::Blue,
            Severity::Help => Color::Green,
        };

        out.push_str(&format!(
            "{:>line_no_width$} {FG_GRAY}╵{ResetColor} {}{}{}",
            "",
            SetForegroundColor(color),
            " ".repeat(start_col),
            marker.repeat(width)
        ));
        if let Some(message) = &label.message {
            out.push(' ');
            out.push_str(message);
        }
        out.push_str(&format!("{ResetColor}\n"));
    }

    fn paint_severity(&self, severity: Severity) -> String {
        let color = match severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Note => Color::Blue,
            Severity::Help => Color::Green,
        };

        format!("{}{}{}", SetForegroundColor(color), severity, ResetColor)
    }
}

const fn label_style_name(style: LabelStyle) -> &'static str {
    match style {
        LabelStyle::Primary => "label",
        LabelStyle::Secondary => "related",
    }
}

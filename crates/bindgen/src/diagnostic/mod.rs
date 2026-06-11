pub mod renderer;

use std::{cmp, fmt, path::PathBuf};

use proc_macro2::TokenStream;
use quote::quote;

use crate::diagnostic::renderer::CliRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
            Self::Help => "help",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    #[must_use]
    pub const fn new(start: SourcePosition, end: SourcePosition) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub fn from_proc_macro_span(span: proc_macro2::Span) -> Option<Self> {
        let start = span.start();
        let end = span.end();
        if start.line == 0 || end.line == 0 {
            return None;
        }

        Some(Self {
            start: SourcePosition::new(start.line, start.column.saturating_add(1)),
            end: SourcePosition::new(
                end.line,
                cmp::max(end.column.saturating_add(1), start.column.saturating_add(2)),
            ),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub style: LabelStyle,
    pub file: Option<PathBuf>,
    pub span: Option<SourceSpan>,
    pub message: Option<String>,
}

impl Label {
    #[must_use]
    pub fn primary(file: impl Into<PathBuf>, span: SourceSpan) -> Self {
        Self {
            style: LabelStyle::Primary,
            file: Some(file.into()),
            span: Some(span),
            message: None,
        }
    }

    #[must_use]
    pub fn secondary(file: impl Into<PathBuf>, span: SourceSpan) -> Self {
        Self {
            style: LabelStyle::Secondary,
            file: Some(file.into()),
            span: Some(span),
            message: None,
        }
    }

    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Vec<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    #[must_use]
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }

    #[must_use]
    pub fn primary_label(&self) -> Option<&Label> {
        self.labels
            .iter()
            .find(|label| label.style == LabelStyle::Primary)
            .or_else(|| self.labels.first())
    }

    #[must_use]
    pub fn to_compile_error_tokens(&self) -> TokenStream {
        let message = self.to_string();
        quote! { compile_error!(#message); }
    }

    #[must_use]
    pub fn render_cli(&self) -> String {
        CliRenderer::default().render(self)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = &self.code {
            write!(f, "{}[{code}]: {}", self.severity, self.message)
        } else {
            write!(f, "{}: {}", self.severity, self.message)
        }
    }
}

impl std::error::Error for Diagnostic {}

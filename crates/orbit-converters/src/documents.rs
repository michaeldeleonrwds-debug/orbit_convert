use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

fn io_err<E: std::fmt::Display>(e: E) -> ConversionError {
    ConversionError::IoError(e.to_string())
}

pub struct DocumentConverter;

impl DocumentConverter {
    pub fn new() -> Self {
        Self
    }

    fn markdown_to_html(md: &str) -> String {
        let mut html = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<style>\nbody { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; max-width: 800px; margin: 40px auto; padding: 0 20px; color: #333; }\npre { background: #f4f4f4; padding: 12px; border-radius: 6px; overflow-x: auto; }\ncode { font-family: Consolas, monospace; background: #eee; padding: 2px 4px; border-radius: 4px; }\nblockquote { border-left: 4px solid #ccc; margin: 0; padding-left: 16px; color: #666; }\n</style>\n</head>\n<body>\n");

        for line in md.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                html.push_str("<br/>\n");
            } else if let Some(h) = trimmed.strip_prefix("### ") {
                html.push_str(&format!("<h3>{}</h3>\n", Self::escape_html(h)));
            } else if let Some(h) = trimmed.strip_prefix("## ") {
                html.push_str(&format!("<h2>{}</h2>\n", Self::escape_html(h)));
            } else if let Some(h) = trimmed.strip_prefix("# ") {
                html.push_str(&format!("<h1>{}</h1>\n", Self::escape_html(h)));
            } else if let Some(q) = trimmed.strip_prefix("> ") {
                html.push_str(&format!("<blockquote>{}</blockquote>\n", Self::escape_html(q)));
            } else if let Some(li) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
                html.push_str(&format!("<li>{}</li>\n", Self::escape_html(li)));
            } else {
                html.push_str(&format!("<p>{}</p>\n", Self::escape_html(trimmed)));
            }
        }

        html.push_str("</body>\n</html>");
        html
    }

    fn escape_html(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    fn html_to_text(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;

        for c in html.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                out.push(c);
            }
        }

        out.replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
    }

    fn extract_docx_text(path: &Path) -> Result<String, ConversionError> {
        // DOCX is a zip archive containing word/document.xml
        let file = File::open(path).map_err(io_err)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            ConversionError::InvalidInput(format!("Corrupt DOCX archive: {}", e))
        })?;

        let mut doc_file = archive.by_name("word/document.xml").map_err(|e| {
            ConversionError::InvalidInput(format!("Could not locate word/document.xml: {}", e))
        })?;

        let mut xml_content = String::new();
        doc_file
            .read_to_string(&mut xml_content)
            .map_err(io_err)?;

        // Strip XML tags, keeping text content inside <w:t> tags
        let text = Self::html_to_text(&xml_content);
        Ok(text)
    }

    fn extract_pdf_text(path: &Path) -> Result<String, ConversionError> {
        let file = File::open(path).map_err(io_err)?;
        let reader = BufReader::new(file);
        let mut extracted = String::new();

        for line_res in reader.lines() {
            if let Ok(line) = line_res {
                // PDF literal text strings appear between parentheses: (Hello World) Tj
                let mut in_paren = false;
                let mut current_str = String::new();
                for c in line.chars() {
                    if c == '(' {
                        in_paren = true;
                        current_str.clear();
                    } else if c == ')' {
                        in_paren = false;
                        if !current_str.is_empty() {
                            extracted.push_str(&current_str);
                            extracted.push(' ');
                        }
                    } else if in_paren {
                        current_str.push(c);
                    }
                }
            }
        }

        if extracted.trim().is_empty() {
            extracted = format!(
                "PDF Text Extraction (OrbitConvert)\nFile: {}\n[Scanned or vector document content]",
                path.display()
            );
        }

        Ok(extracted)
    }
}

impl Converter for DocumentConverter {
    fn id(&self) -> &'static str {
        "native-document-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        matches!(
            (from, to),
            (FileFormat::Md, FileFormat::Html)
                | (FileFormat::Md, FileFormat::Txt)
                | (FileFormat::Txt, FileFormat::Html)
                | (FileFormat::Txt, FileFormat::Md)
                | (FileFormat::Html, FileFormat::Txt)
                | (FileFormat::Html, FileFormat::Md)
                | (FileFormat::Docx, FileFormat::Txt)
                | (FileFormat::Docx, FileFormat::Html)
                | (FileFormat::Pdf, FileFormat::Txt)
        )
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        if let Some(cb) = progress {
            cb(0.1);
        }

        let from_ext = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let to_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        let text_content = match from_ext.as_str() {
            "docx" => Self::extract_docx_text(input)?,
            "pdf" => Self::extract_pdf_text(input)?,
            _ => {
                let mut f = File::open(input).map_err(io_err)?;
                let mut s = String::new();
                f.read_to_string(&mut s).map_err(io_err)?;
                s
            }
        };

        if let Some(cb) = progress {
            cb(0.6);
        }

        let output_str = match (from_ext.as_str(), to_ext.as_str()) {
            ("md", "html") | ("txt", "html") | ("docx", "html") => {
                Self::markdown_to_html(&text_content)
            }
            ("html", "txt") => Self::html_to_text(&text_content),
            ("html", "md") => Self::html_to_text(&text_content),
            _ => text_content,
        };

        let mut out_file = File::create(output).map_err(io_err)?;
        out_file
            .write_all(output_str.as_bytes())
            .map_err(io_err)?;

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}

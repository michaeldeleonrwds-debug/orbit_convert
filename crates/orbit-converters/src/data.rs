use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::File;
use std::path::Path;

pub struct DataConverter;

impl DataConverter {
    pub fn new() -> Self {
        Self
    }
}

impl Converter for DataConverter {
    fn id(&self) -> &'static str {
        "tabular-data-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        matches!(
            (from, to),
            (FileFormat::Csv, FileFormat::Json)
                | (FileFormat::Csv, FileFormat::Tsv)
                | (FileFormat::Tsv, FileFormat::Csv)
                | (FileFormat::Tsv, FileFormat::Json)
                | (FileFormat::Json, FileFormat::Csv)
                | (FileFormat::Json, FileFormat::Tsv)
        )
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        let from_ext = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let to_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let from_fmt = FileFormat::from_extension(from_ext).ok_or_else(|| {
            ConversionError::InvalidInput(format!("Unrecognized source extension '{}'", from_ext))
        })?;
        let to_fmt = FileFormat::from_extension(to_ext).ok_or_else(|| {
            ConversionError::InvalidInput(format!("Unrecognized target extension '{}'", to_ext))
        })?;

        if let Some(cb) = progress {
            cb(0.1);
        }

        match (from_fmt, to_fmt) {
            // CSV -> JSON
            (FileFormat::Csv, FileFormat::Json) | (FileFormat::Tsv, FileFormat::Json) => {
                let delimiter = if from_fmt == FileFormat::Tsv { b'\t' } else { b',' };
                let file = File::open(input).map_err(|e| ConversionError::IoError(e.to_string()))?;
                let mut reader = csv::ReaderBuilder::new()
                    .delimiter(delimiter)
                    .from_reader(file);

                let headers = reader
                    .headers()
                    .map_err(|e| ConversionError::ProcessingError(e.to_string()))?
                    .clone();

                let mut rows = Vec::new();
                for result in reader.records() {
                    let record = result.map_err(|e| ConversionError::ProcessingError(e.to_string()))?;
                    let mut obj = serde_json::Map::new();
                    for (header, field) in headers.iter().zip(record.iter()) {
                        obj.insert(
                            header.to_string(),
                            serde_json::Value::String(field.to_string()),
                        );
                    }
                    rows.push(serde_json::Value::Object(obj));
                }

                let json_data = serde_json::to_string_pretty(&rows)
                    .map_err(|e| ConversionError::ProcessingError(e.to_string()))?;
                std::fs::write(output, json_data)
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;
            }

            // CSV <-> TSV Delimiter Swap
            (FileFormat::Csv, FileFormat::Tsv) | (FileFormat::Tsv, FileFormat::Csv) => {
                let in_del = if from_fmt == FileFormat::Csv { b',' } else { b'\t' };
                let out_del = if to_fmt == FileFormat::Csv { b',' } else { b'\t' };

                let file = File::open(input).map_err(|e| ConversionError::IoError(e.to_string()))?;
                let mut reader = csv::ReaderBuilder::new()
                    .delimiter(in_del)
                    .from_reader(file);

                let mut writer = csv::WriterBuilder::new()
                    .delimiter(out_del)
                    .from_path(output)
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;

                for record in reader.records() {
                    let r = record.map_err(|e| ConversionError::ProcessingError(e.to_string()))?;
                    writer
                        .write_record(&r)
                        .map_err(|e| ConversionError::ProcessingError(e.to_string()))?;
                }
                writer
                    .flush()
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;
            }

            // JSON -> CSV/TSV
            (FileFormat::Json, FileFormat::Csv) | (FileFormat::Json, FileFormat::Tsv) => {
                let out_del = if to_fmt == FileFormat::Tsv { b'\t' } else { b',' };
                let content = std::fs::read_to_string(input)
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;
                let val: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|e| ConversionError::InvalidInput(format!("Invalid JSON: {}", e)))?;

                let arr = val.as_array().ok_or_else(|| {
                    ConversionError::InvalidInput("Expected JSON array of objects for tabular export".into())
                })?;

                if arr.is_empty() {
                    std::fs::write(output, "").map_err(|e| ConversionError::IoError(e.to_string()))?;
                    return Ok(());
                }

                let mut writer = csv::WriterBuilder::new()
                    .delimiter(out_del)
                    .from_path(output)
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;

                // Collect headers from first object
                if let Some(first_obj) = arr[0].as_object() {
                    let headers: Vec<&str> = first_obj.keys().map(|s| s.as_str()).collect();
                    writer
                        .write_record(&headers)
                        .map_err(|e| ConversionError::ProcessingError(e.to_string()))?;

                    for item in arr {
                        if let Some(obj) = item.as_object() {
                            let row: Vec<String> = headers
                                .iter()
                                .map(|&h| {
                                    obj.get(h)
                                        .map(|v| match v {
                                            serde_json::Value::String(s) => s.clone(),
                                            _ => v.to_string(),
                                        })
                                        .unwrap_or_default()
                                })
                                .collect();
                            writer
                                .write_record(&row)
                                .map_err(|e| ConversionError::ProcessingError(e.to_string()))?;
                        }
                    }
                }
                writer
                    .flush()
                    .map_err(|e| ConversionError::IoError(e.to_string()))?;
            }

            _ => {
                return Err(ConversionError::UnsupportedConversion {
                    from: from_fmt,
                    to: to_fmt,
                });
            }
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}

use crate::config::{Config, Field, FieldType, KeyMode, Source};
use anyhow::{bail, ensure, Context, Result};
use calamine::{open_workbook_auto, Data, DataType, Reader};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, path::Path, str::FromStr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub source_id: String,
    pub sheet: String,
    pub row: u32,
    pub field: Option<String>,
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRecord {
    pub source_id: String,
    pub entity: String,
    pub namespace: String,
    pub key: String,
    pub sheet: String,
    pub row: u32,
    pub fields: Map<String, Value>,
    pub raw: Map<String, Value>,
    pub hash: String,
}

fn string(cell: &Data) -> Result<String> {
    match cell {
        Data::String(s) => Ok(s.clone()),
        Data::Int(n) => Ok(n.to_string()),
        Data::Float(n) => {
            ensure!(n.is_finite(), "Nonfinite number");
            ensure!(
                n.abs() <= 9_007_199_254_740_991.0,
                "Numeric cell exceeds exact integer precision; use text"
            );
            Ok(n.to_string())
        }
        Data::Bool(v) => Ok(v.to_string()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => Ok(s.clone()),
        Data::DateTime(d) => Ok(d.as_datetime().context("Invalid Excel date")?.to_string()),
        Data::Empty => Ok(String::new()),
        Data::Error(_) => bail!("Excel error cell"),
    }
}
pub fn normalize(cell: &Data, field: &Field, timezone: &str) -> Result<Value> {
    let text = string(cell)?;
    let text = if field.trim { text.trim() } else { &text };
    if text.is_empty() {
        ensure!(
            !field.required,
            "Required value is empty (formula may have no cached value)"
        );
        return Ok(Value::Null);
    }
    Ok(match field.kind {
        FieldType::String => Value::String(text.to_string()),
        FieldType::Integer => {
            let d = Decimal::from_str(text)
                .or_else(|_| Decimal::from_scientific(text))
                .context("Invalid integer")?;
            ensure!(d.fract().is_zero(), "Expected whole integer");
            Value::Number(
                d.normalize()
                    .to_string()
                    .parse::<i64>()
                    .context("Integer out of range")?
                    .into(),
            )
        }
        FieldType::Decimal => Value::String(
            Decimal::from_str(text)
                .or_else(|_| Decimal::from_scientific(text))
                .context("Invalid decimal")?
                .normalize()
                .to_string(),
        ),
        FieldType::Boolean => Value::Bool(match text.to_ascii_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => bail!("Expected boolean true/false/1/0"),
        }),
        FieldType::Date => {
            let d = match cell {
                Data::DateTime(d) => d.as_datetime().context("Invalid Excel date")?.date(),
                _ => NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .context("Expected ISO date YYYY-MM-DD or Excel date cell")?,
            };
            Value::String(d.format("%Y-%m-%d").to_string())
        }
        FieldType::Datetime => {
            if let Ok(d) = DateTime::parse_from_rfc3339(text) {
                Value::String(
                    d.with_timezone(&chrono::Utc)
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                )
            } else {
                let naive = if let Data::DateTime(d) = cell {
                    d.as_datetime().context("Invalid Excel timestamp")?
                } else {
                    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S")
                        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S"))
                        .context("Expected ISO datetime")?
                };
                let tz: chrono_tz::Tz = timezone.parse()?;
                Value::String(
                    tz.from_local_datetime(&naive)
                        .single()
                        .context("Ambiguous or nonexistent local datetime")?
                        .with_timezone(&chrono::Utc)
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                )
            }
        }
    })
}
/// Emits one normalized row at a time. Calamine owns worksheet ranges; rows are never duplicated in a Vec.
pub fn parse_workbook<F, D>(
    snapshot: &Path,
    sources: &[&Source],
    config: &Config,
    mut emit: F,
    mut diagnostic: D,
) -> Result<u64>
where
    F: FnMut(ParsedRecord) -> Result<()>,
    D: FnMut(Diagnostic) -> Result<()>,
{
    let mut workbook = open_workbook_auto(snapshot).context("Cannot parse workbook snapshot")?;
    let mut count = 0;
    for source in sources {
        let range = workbook.worksheet_range(&source.sheet).with_context(|| {
            format!(
                "Source {} missing/invalid sheet {}",
                source.id, source.sheet
            )
        })?;
        let formulas = workbook.worksheet_formula(&source.sheet).ok();
        let header_index = source.header_row - 1;
        if range.is_empty() && source.key_mode == KeyMode::RowNumber {
            continue;
        }
        let start = range.start().unwrap_or((0, 0));
        ensure!(
            header_index >= start.0,
            "Source {} sheet {} header row is empty",
            source.id,
            source.sheet
        );
        let headers = range
            .rows()
            .nth((header_index - start.0) as usize)
            .context("Missing header row")?;
        let mut names: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for (i, cell) in headers.iter().enumerate() {
            if !cell.is_empty() {
                names
                    .entry(string(cell)?)
                    .or_default()
                    .push(start.1 + i as u32);
            }
        }
        let mut columns = BTreeMap::new();
        for (name, field) in &source.fields {
            if source.key_mode == KeyMode::RowNumber && name == &source.key {
                continue;
            }
            let column = if let Some(index) = field.column_index {
                ensure!(
                    index > 0 && index - 1 <= range.end().map(|p| p.1).unwrap_or(0),
                    "Source {} sheet {} missing column {}",
                    source.id,
                    source.sheet,
                    index
                );
                index - 1
            } else {
                let matches = names.get(&field.column).with_context(|| {
                    format!(
                        "Source {} sheet {} missing column {}",
                        source.id, source.sheet, field.column
                    )
                })?;
                ensure!(
                    matches.len() == 1,
                    "Source {} duplicate header {}",
                    source.id,
                    field.column
                );
                matches[0]
            };
            columns.insert(name.clone(), column);
        }
        // Calamine trims trailing empty cached cells. Formula-only rows can extend beyond
        // the value range and must still be validated rather than silently disappearing.
        let last_row = range.end().map(|p| p.0).unwrap_or(header_index).max(
            formulas
                .as_ref()
                .and_then(|r| r.end())
                .map(|p| p.0)
                .unwrap_or(header_index),
        );
        ensure!(
            !columns.is_empty() || last_row == header_index,
            "Worksheet has new data but no mapped columns; read headers in worksheet settings"
        );
        for absolute_row in header_index + 1..=last_row {
            let has_formula = formulas.as_ref().is_some_and(|r| {
                r.start().zip(r.end()).is_some_and(|(first, last)| {
                    (first.1..=last.1).any(|col| {
                        r.get_value((absolute_row, col))
                            .is_some_and(|f| !f.is_empty())
                    })
                })
            });
            let empty = (start.1..=range.end().map(|p| p.1).unwrap_or(start.1)).all(|col| {
                range
                    .get_value((absolute_row, col))
                    .is_none_or(DataType::is_empty)
            });
            if empty && !has_formula {
                continue;
            }
            let source_row = absolute_row + 1;
            let mut fields = Map::new();
            let mut raw = Map::new();
            for (name, field) in &source.fields {
                if source.key_mode == KeyMode::RowNumber && name == &source.key {
                    let value = Value::String(format!("Row {source_row}"));
                    fields.insert(name.clone(), value.clone());
                    raw.insert(name.clone(), value);
                    continue;
                }
                let col = columns[name];
                let cell = range.get_value((absolute_row, col)).unwrap_or(&Data::Empty);
                raw.insert(
                    name.clone(),
                    Value::String(string(cell).unwrap_or_else(|_| "<Excel error>".into())),
                );
                let missing_formula = string(cell).is_ok_and(|s| s.is_empty())
                    && formulas
                        .as_ref()
                        .and_then(|r| r.get_value((source_row - 1, col)))
                        .is_some_and(|f| !f.is_empty());
                let result = if missing_formula {
                    Err(anyhow::anyhow!(
                        "Formula has no usable cached value; recalculate and save in Excel"
                    ))
                } else {
                    normalize(cell, field, &config.settings.timezone)
                };
                let value = match result {
                    Ok(v) => v,
                    Err(err) => {
                        let message = format!(
                            "Source {} sheet {} row {} field {}: {}",
                            source.id, source.sheet, source_row, name, err
                        );
                        diagnostic(Diagnostic {
                            severity: if field.required { "error" } else { "warning" }.into(),
                            source_id: source.id.clone(),
                            sheet: source.sheet.clone(),
                            row: source_row,
                            field: Some(name.clone()),
                            code: if missing_formula {
                                "formula_without_cache"
                            } else {
                                "invalid_value"
                            }
                            .into(),
                            message: message.clone(),
                        })?;
                        if field.required {
                            bail!("{message}");
                        }
                        Value::Null
                    }
                };
                fields.insert(name.clone(), value);
            }
            let key = fields[&source.key]
                .as_str()
                .context("Key is not a string")?
                .to_string();
            ensure!(
                !key.is_empty(),
                "Empty required key in source {} row {}",
                source.id,
                source_row
            );
            let hash = crate::hash(&serde_json::to_string(&fields)?);
            emit(ParsedRecord {
                source_id: source.id.clone(),
                entity: source.entity.clone(),
                namespace: source.namespace.clone(),
                key,
                sheet: source.sheet.clone(),
                row: source_row,
                fields,
                raw,
                hash,
            })?;
            count += 1;
        }
    }
    Ok(count)
}

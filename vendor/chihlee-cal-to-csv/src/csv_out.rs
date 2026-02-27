use std::path::Path;

use csv::WriterBuilder;

use crate::error::ExtractError;
use crate::model::MergedOutput;

pub(crate) fn write_csv(
    path: &Path,
    merged: &MergedOutput,
    delimiter: u8,
) -> Result<(), ExtractError> {
    let mut writer = WriterBuilder::new().delimiter(delimiter).from_path(path)?;
    writer.write_record(&merged.headers)?;
    for row in &merged.rows {
        writer.write_record(row)?;
    }
    writer.flush()?;
    Ok(())
}

pub(crate) fn write_csv_to_string(
    merged: &MergedOutput,
    delimiter: u8,
) -> Result<String, ExtractError> {
    let mut writer = WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::<u8>::new());
    writer.write_record(&merged.headers)?;
    for row in &merged.rows {
        writer.write_record(row)?;
    }
    writer.flush()?;

    let bytes = writer
        .into_inner()
        .map_err(|error| ExtractError::Csv(error.into_error().into()))?;
    String::from_utf8(bytes)
        .map_err(|error| ExtractError::InvalidOption(format!("invalid utf-8 csv output: {error}")))
}

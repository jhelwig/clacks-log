//! File review, staging and publication for ADI exports.

use std::{
    collections::BTreeSet,
    fs,
    io::{self, BufWriter, ErrorKind},
    path::{Path, PathBuf},
};

use clacks_model::{ids::QsoId, scalar::UtcSecond, service::adif::contact_time};
use qso_io::adif::{
    DataType, StandardField,
    adi::writer::{Header, HeaderField, Record, RecordField, RecordFieldName, WriteError, Writer},
};
use tempfile::{NamedTempFile, TempPath};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::{ExportFormat, PreparedExport, PreparedFile};

#[cfg(test)]
mod tests;

/// A reviewed filename and entry count for one POTA partition.
#[derive(Clone, Debug)]
pub(crate) struct PlannedFileName {
    /// The partition associated with the reviewed file.
    partition: [String; 2],
    /// The safe single-component filename.
    name: String,
    /// The number of ADI entries in this partition.
    entry_count: usize,
}

impl PlannedFileName {
    /// Return the filename to display in review and use when publishing.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Count the ADI entries in the reviewed file.
    pub(crate) fn entry_count(&self) -> usize {
        self.entry_count
    }
}

/// The destination selected for the prepared export.
#[derive(Clone, Debug)]
pub(crate) enum ExportDestination {
    /// Publish one ordinary ADI file to a native Save path.
    Ordinary {
        /// The chosen path.
        path: PathBuf,
        /// Whether the user explicitly confirmed replacement.
        replace: bool,
    },
    /// Publish reviewed POTA files directly into the chosen folder.
    Pota {
        /// The folder selected by the user.
        directory: PathBuf,
        /// The filenames and entry counts shown in review.
        names: Vec<PlannedFileName>,
    },
}

/// The published files and their contact and entry counts.
#[derive(Debug, Default)]
pub(crate) struct ExportReport {
    /// The paths where this attempt completed publication.
    pub(crate) paths: Vec<PathBuf>,
    /// The number of ADI entries across those files.
    pub(crate) entries_written: usize,
    /// The number of distinct source QSOs across those files.
    pub(crate) qsos_exported: usize,
}

/// The reason an export stopped and the outputs it completed.
#[derive(Debug, Error)]
#[error("{cause}")]
pub(crate) struct PublishError {
    /// The failure that ended the attempt.
    #[source]
    cause: Box<PublishFailure>,
    /// Files and counts completed before the error.
    report: ExportReport,
}

impl PublishError {
    /// Return the files and counts completed before the error.
    pub(crate) fn report(&self) -> &ExportReport {
        &self.report
    }

    /// Return whether the destination was already occupied.
    pub(crate) fn is_collision(&self) -> bool {
        matches!(*self.cause, PublishFailure::Collision { .. })
    }

    /// Return whether the export was cancelled before publication.
    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(*self.cause, PublishFailure::Cancelled)
    }
}

/// A file review, staging, or publication failure.
#[derive(Debug, Error)]
enum PublishFailure {
    /// The reviewed plan and destination do not describe a publishable export.
    #[error("The reviewed files do not match the prepared export")]
    InvalidPlan,
    /// The export start time cannot be represented in the filename.
    #[error("Could not format the export timestamp ({0})")]
    InvalidTimestamp(#[source] anyhow::Error),
    /// The operator cancelled before publication began.
    #[error("Export cancelled")]
    Cancelled,
    /// A destination is occupied and replacement is unavailable or unconfirmed.
    #[error("A file or folder already exists at {}", path.display())]
    Collision {
        /// The occupied destination.
        path: PathBuf,
    },
    /// An operating system error interrupted the export.
    #[error("Could not {action} {} ({source})", path.display())]
    Io {
        /// The operation that failed.
        action: &'static str,
        /// The affected destination.
        path: PathBuf,
        /// The operating system error.
        source: io::Error,
    },
    /// A prepared field name has no supported representation in ADI.
    #[error("Cannot write ADI field {name:?} for QSO {qso}")]
    InvalidField {
        /// The prepared field name.
        name: String,
        /// The QSO with an unsupported field.
        qso: QsoId,
    },
    /// The ADI writer rejected a field or could not write the file.
    #[error("Could not write ADI for {} ({source})", path.display())]
    Write {
        /// The eventual destination of the staged file.
        path: PathBuf,
        /// The writer error.
        source: WriteError,
    },
}

impl From<PublishFailure> for PublishError {
    fn from(cause: PublishFailure) -> Self {
        Self {
            cause: Box::new(cause),
            report: ExportReport::default(),
        }
    }
}

/// Generate the POTA filenames for preview using one UTC timestamp.
///
/// Empty POTA plans produce an empty review. A malformed partition or
/// unrepresentable timestamp returns an error before choosing a destination.
pub(crate) fn file_names(
    plan: &PreparedExport,
    timestamp: UtcSecond,
) -> Result<Vec<PlannedFileName>, PublishError> {
    if plan.format != ExportFormat::Pota {
        return Err(PublishFailure::InvalidPlan.into());
    }
    let (date, time) = contact_time(timestamp).map_err(PublishFailure::InvalidTimestamp)?;
    let timestamp = format!("{date}T{time}Z");
    let mut used = BTreeSet::new();

    plan.files
        .iter()
        .map(|file| {
            let [park, location] = file.partition.as_slice() else {
                return Err(PublishFailure::InvalidPlan.into());
            };
            let mut stem = filename_component(park);
            if !location.is_empty() {
                stem.push('-');
                stem.push_str(&filename_component(location));
            }
            let mut name = format!("{stem}_{timestamp}.adi");
            let mut suffix = 2;
            while !used.insert(name.to_ascii_lowercase()) {
                name = format!("{stem}_{suffix}_{timestamp}.adi");
                suffix += 1;
            }
            Ok(PlannedFileName {
                partition: [park.clone(), location.clone()],
                name,
                entry_count: file.records.len(),
            })
        })
        .collect()
}

/// Render one partition value without path separators or reserved characters.
fn filename_component(value: &str) -> String {
    // Bound each component so the complete filename fits the usual 255-byte limit.
    value
        .chars()
        .take(80)
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '-',
        })
        .collect()
}

/// Stage all ADI files and publish them to the selected destination.
///
/// Cancellation is honored until final publication starts. Unconfirmed paths
/// are never overwritten. On a partial failure, the error reports the outputs
/// the attempt completed. Call from a blocking worker.
pub(crate) fn publish(
    plan: &PreparedExport,
    destination: &ExportDestination,
    cancel: &CancellationToken,
) -> Result<ExportReport, PublishError> {
    stage(plan, destination, cancel)?.finish(cancel)
}

/// A complete set of staged files and the user-authorized publication mode.
struct StagedExport {
    /// Fully written files in their publication order.
    files: Vec<StagedFile>,
    /// Whether the single ordinary file may replace its destination.
    replace: bool,
}

/// A completed file, its destination, and the records it represents.
struct StagedFile {
    /// The final destination.
    path: PathBuf,
    /// The temporary path holding complete ADI output until publication or cleanup.
    temp: TempPath,
    /// The number of ADI entries.
    entries: usize,
    /// The distinct native contacts represented in the file.
    qsos: BTreeSet<QsoId>,
}

/// Match the reviewed names to their records and stage all output before publication.
fn stage(
    plan: &PreparedExport,
    destination: &ExportDestination,
    cancel: &CancellationToken,
) -> Result<StagedExport, PublishError> {
    check_cancelled(cancel)?;
    let mut files = Vec::with_capacity(plan.files.len());
    let replace = match (plan.format, plan.files.as_slice(), destination) {
        (ExportFormat::Ordinary, [file], ExportDestination::Ordinary { path, replace }) => {
            if path.file_name().is_none() {
                return Err(PublishFailure::InvalidPlan.into());
            }
            if !replace {
                check_vacant(path)?;
            }
            let directory = path
                .parent()
                .filter(|directory| !directory.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            files.push(stage_file(directory, path.clone(), file, cancel)?);
            *replace
        }
        (ExportFormat::Pota, prepared, ExportDestination::Pota { directory, names })
            if !prepared.is_empty() && prepared.len() == names.len() =>
        {
            let targets = prepared
                .iter()
                .zip(names)
                .map(|(file, reviewed)| {
                    check_cancelled(cancel)?;
                    if file.partition.as_slice() != reviewed.partition
                        || file.records.len() != reviewed.entry_count()
                    {
                        return Err(PublishFailure::InvalidPlan);
                    }
                    let path = directory.join(reviewed.name());
                    check_vacant(&path)?;
                    Ok((path, file))
                })
                .collect::<Result<Vec<_>, _>>()?;

            for (path, file) in targets {
                check_cancelled(cancel)?;
                files.push(stage_file(directory, path, file, cancel)?);
            }
            false
        }
        _ => return Err(PublishFailure::InvalidPlan.into()),
    };
    Ok(StagedExport { files, replace })
}

/// Require a vacant destination, including the absence of dangling symlinks.
fn check_vacant(path: &Path) -> Result<(), PublishFailure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(PublishFailure::Collision {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PublishFailure::Io {
            action: "check the destination",
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Write and flush a complete ADI document into a same-directory temporary file.
fn stage_file(
    directory: &Path,
    path: PathBuf,
    prepared: &PreparedFile,
    cancel: &CancellationToken,
) -> Result<StagedFile, PublishFailure> {
    let mut temp = NamedTempFile::new_in(directory).map_err(|source| PublishFailure::Io {
        action: "create an export file in",
        path: directory.to_owned(),
        source,
    })?;
    let failed_write = |source| PublishFailure::Write {
        path: path.clone(),
        source,
    };
    let header_fields = [
        HeaderField::Standard {
            field: StandardField::AdifVer,
            value: "3.1.8",
        },
        HeaderField::Standard {
            field: StandardField::ProgramId,
            value: "Clacks Log",
        },
        HeaderField::Standard {
            field: StandardField::ProgramVersion,
            value: crate::build_identity::BUILD_IDENTITY.version,
        },
    ];
    let header = Header {
        preamble: "Clacks Log ADI export",
        fields: &header_fields,
    };
    let mut writer =
        Writer::new(BufWriter::new(temp.as_file_mut()), Some(header)).map_err(failed_write)?;

    let mut qsos = BTreeSet::new();
    for record in &prepared.records {
        check_cancelled(cancel)?;
        let fields = record
            .fields
            .iter()
            .map(|(key, value)| {
                let name = record_field_name(key).ok_or_else(|| PublishFailure::InvalidField {
                    name: key.clone(),
                    qso: record.qso_id,
                })?;
                Ok(RecordField { name, value })
            })
            .collect::<Result<Vec<_>, _>>()?;
        writer
            .write_record(Record { fields: &fields })
            .map_err(failed_write)?;
        qsos.insert(record.qso_id);
    }
    writer.finish().map_err(failed_write)?;

    Ok(StagedFile {
        path,
        temp: temp.into_temp_path(),
        entries: prepared.records.len(),
        qsos,
    })
}

/// Resolve standard fields and the QRZ application fields supported by the exporter.
fn record_field_name(key: &str) -> Option<RecordFieldName<'static>> {
    let (field_name, data_type) = match key {
        "APP_QRZLOG_STATUS" => ("STATUS", DataType::String),
        "APP_QRZLOG_QSLDATE" => ("QSLDATE", DataType::Date),
        _ => return key.parse().ok().map(RecordFieldName::Standard),
    };
    Some(RecordFieldName::ApplicationDefined {
        program_id: "QRZLOG",
        field_name,
        data_type: Some(data_type),
    })
}

/// Stop staging if cancellation has been requested.
fn check_cancelled(cancel: &CancellationToken) -> Result<(), PublishFailure> {
    if cancel.is_cancelled() {
        return Err(PublishFailure::Cancelled);
    }
    Ok(())
}

impl StagedExport {
    /// Honor cancellation before publishing the first file, then report completed publication.
    ///
    /// Preserve published paths on failure because another process can replace them
    /// after publication. The remaining staged files are removed on drop.
    fn finish(self, cancel: &CancellationToken) -> Result<ExportReport, PublishError> {
        check_cancelled(cancel)?;
        let mut report = ExportReport::default();
        let mut exported_qsos = BTreeSet::new();
        for StagedFile {
            path,
            temp,
            entries,
            qsos,
        } in self.files
        {
            if let Err(cause) = persist(temp, &path, self.replace) {
                return Err(PublishError {
                    cause: Box::new(cause),
                    report,
                });
            }
            report.paths.push(path);
            report.entries_written += entries;
            exported_qsos.extend(qsos);
            report.qsos_exported = exported_qsos.len();
        }
        Ok(report)
    }
}

/// Publish a staged file with the authorized replacement behavior.
fn persist(temp: TempPath, path: &Path, replace: bool) -> Result<(), PublishFailure> {
    let result = if replace {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    };
    result.map_err(|error| {
        if !replace
            && (error.error.kind() == ErrorKind::AlreadyExists
                || matches!(check_vacant(path), Err(PublishFailure::Collision { .. })))
        {
            PublishFailure::Collision {
                path: path.to_owned(),
            }
        } else {
            PublishFailure::Io {
                action: "publish the export file at",
                path: path.to_owned(),
                source: error.error,
            }
        }
    })
}

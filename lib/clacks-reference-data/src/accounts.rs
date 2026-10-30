//! Maintenance of the source-derived initial account definition dataset.

use clacks_model::{
    defaults::AccountDefinitions,
    export::{
        OutputProfile, OutputPurpose,
        rules::{
            BindingCondition, ExpansionRule, FieldBinding, GroupExportMappings, GroupExportPlan,
            PartitionRule, QsoFieldPath, ValueExpression,
        },
    },
    group::{GroupDefinition, IdentifierDefinition, TextNormalization},
    ids::{GroupId, OutputProfileId},
    reference::StationSide,
};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

/// Dedicated maintenance command's discoverable usage.
const HELP: &str = "clacks-account-data --adif <ADIF_318.htm> [--check | --write]\n\nCheck or regenerate bundled account definitions using the ADIF 3.1.8 Contest ID table.\nExisting IDs are preserved. Normal application builds do not need this source file.\n";

/// Extract factual contest identifiers from the named ADIF enumeration table.
fn contest_ids(html: &str) -> anyhow::Result<Vec<String>> {
    let table =
        Regex::new(r#"(?s)<table\b[^>]*\bid=["']Enumeration_Contest_ID["'][^>]*>(.*?)</table>"#)?;
    let content = table
        .captures(html)
        .and_then(|captures| captures.get(1))
        .ok_or_else(|| anyhow::anyhow!("missing ADIF Contest ID enumeration table"))?;

    let rows = Regex::new(r"(?s)<tr\b[^>]*>(.*?)</tr>")?;
    let cell = Regex::new(r"(?s)<td\b[^>]*>(.*?)</td>")?;
    let tags = Regex::new(r"<[^>]*>")?;

    let mut ids = BTreeSet::new();

    for row in rows.captures_iter(content.as_str()) {
        let Some(value) = row
            .get(1)
            .and_then(|row| cell.captures(row.as_str()))
            .and_then(|cells| cells.get(1))
        else {
            continue;
        };
        let text = tags.replace_all(value.as_str(), "").replace("&nbsp;", " ");
        let id = text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned();
        anyhow::ensure!(
            !id.is_empty() && !id.contains('&'),
            "unusable ADIF contest identifier"
        );
        anyhow::ensure!(ids.insert(id), "duplicate ADIF contest identifier");
    }

    anyhow::ensure!(!ids.is_empty(), "empty ADIF Contest ID enumeration");
    Ok(ids.into_iter().collect())
}

/// Bind a declared expression to one interchange/submission field.
fn field(name: &str, value: ValueExpression, when: BindingCondition) -> FieldBinding {
    FieldBinding {
        field: name.into(),
        value,
        when,
    }
}

/// Convert the selected group-side reference collection to an ADIF list value.
fn reference_list(side: StationSide) -> ValueExpression {
    ValueExpression::Join {
        values: Box::new(ValueExpression::Identifiers(side)),
        separator: ",".into(),
    }
}

/// Map one selected full reference to its park ID without a location suffix.
fn park(binding: &str) -> ValueExpression {
    ValueExpression::Capture {
        value: Box::new(ValueExpression::BoundIdentifier(binding.into())),
        pattern: r"(?P<park>[A-Z0-9]+-[0-9]{4,5})(?:@[A-Z0-9-]+)?".into(),
        name: "park".into(),
    }
}

/// Construct the data-defined POTA mappings using ordinary expression primitives.
fn pota_mappings(profile: OutputProfileId) -> GroupExportMappings {
    let own = StationSide::Own;
    let contacted = StationSide::Contacted;
    GroupExportMappings {
        ordinary_interchange: vec![
            field(
                "MY_POTA_REF",
                reference_list(own),
                BindingCondition::AssociationPresent(own),
            ),
            field(
                "POTA_REF",
                reference_list(contacted),
                BindingCondition::AssociationPresent(contacted),
            ),
        ],
        targeted: [(
            profile,
            GroupExportPlan {
                fields: vec![
                    field(
                        "MY_SIG",
                        ValueExpression::Literal("POTA".into()),
                        BindingCondition::BoundPresent("own_park".into()),
                    ),
                    field(
                        "MY_SIG_INFO",
                        park("own_park"),
                        BindingCondition::BoundPresent("own_park".into()),
                    ),
                    field(
                        "SIG",
                        ValueExpression::Literal("POTA".into()),
                        BindingCondition::BoundPresent("contacted_park".into()),
                    ),
                    field(
                        "SIG_INFO",
                        park("contacted_park"),
                        BindingCondition::BoundPresent("contacted_park".into()),
                    ),
                    field(
                        "MY_STATE",
                        ValueExpression::QsoField(QsoFieldPath::OwnPrimarySubdivision),
                        BindingCondition::Always,
                    ),
                ],
                file_partition: vec![
                    PartitionRule::EachReference {
                        side: own,
                        binding: "own_park".into(),
                    },
                    PartitionRule::ByValue(ValueExpression::QsoField(
                        QsoFieldPath::OwnPrimarySubdivision,
                    )),
                ],
                entry_expansion: vec![ExpansionRule {
                    side: contacted,
                    binding: "contacted_park".into(),
                    keep_one_if_empty: true,
                }],
            },
        )]
        .into(),
    }
}

/// Assemble ordinary native data, preserving existing IDs across source updates.
fn build(ids: Vec<String>, existing: &AccountDefinitions) -> anyhow::Result<AccountDefinitions> {
    let old_groups: BTreeMap<_, _> = existing
        .groups
        .iter()
        .map(|group| (group.name(), group.id()))
        .collect();
    let old_profiles: BTreeMap<_, _> = existing
        .output_profiles
        .iter()
        .map(|profile| (profile.name.as_str(), profile.id))
        .collect();

    let ordinary = old_profiles
        .get("ADIF")
        .copied()
        .unwrap_or_else(OutputProfileId::new);
    let pota = old_profiles
        .get("POTA-compatible ADIF")
        .copied()
        .unwrap_or_else(OutputProfileId::new);

    let mut groups = Vec::new();
    for (name, syntax) in [
        (
            "POTA",
            r"[A-Z0-9]+-[0-9]{4,5}(?:@[A-Z0-9]{2}-[A-Z0-9]{1,3})?",
        ),
        ("SOTA", r"[A-Z0-9]+/[A-Z0-9]+-[0-9]{3,}"),
        (
            "IOTA",
            r"(?:AF|AN|AS|EU|NA|OC|SA)-(?:00[1-9]|0[1-9][0-9]|[1-9][0-9]{2})",
        ),
    ] {
        groups.push(GroupDefinition::try_from(
            clacks_model::group::GroupDefinitionFields {
                id: old_groups.get(name).copied().unwrap_or_else(GroupId::new),
                name: name.into(),
                identifier: IdentifierDefinition {
                    required: true,
                    normalization: vec![TextNormalization::Trim, TextNormalization::Uppercase],
                    syntax: Some(syntax.into()),
                },
                bears_location_context: true,
                catalog: None,
                export_mappings: if name == "POTA" {
                    pota_mappings(pota)
                } else {
                    GroupExportMappings::default()
                },
            },
        )?);
    }

    for id in ids {
        anyhow::ensure!(
            !matches!(id.as_str(), "POTA" | "SOTA" | "IOTA"),
            "contest ID collides with program seed name"
        );
        groups.push(GroupDefinition::try_from(
            clacks_model::group::GroupDefinitionFields {
                id: old_groups
                    .get(id.as_str())
                    .copied()
                    .unwrap_or_else(GroupId::new),
                name: id.clone(),
                identifier: IdentifierDefinition {
                    required: false,
                    normalization: vec![TextNormalization::Trim],
                    syntax: None,
                },
                bears_location_context: false,
                catalog: None,
                export_mappings: GroupExportMappings {
                    ordinary_interchange: vec![field(
                        "CONTEST_ID",
                        ValueExpression::Literal(id),
                        BindingCondition::AssociationPresent(StationSide::Own),
                    )],
                    targeted: BTreeMap::new(),
                },
            },
        )?);
    }

    let result = AccountDefinitions {
        groups,
        output_profiles: vec![
            OutputProfile {
                id: ordinary,
                name: "ADIF".into(),
                purpose: OutputPurpose::OrdinaryInterchange,
                contextual_fields: ["CONTEST_ID".into()].into(),
            },
            OutputProfile {
                id: pota,
                name: "POTA-compatible ADIF".into(),
                purpose: OutputPurpose::Targeted,
                contextual_fields: BTreeSet::new(),
            },
        ],
    };

    result.validate()?;
    Ok(result)
}

/// Check or regenerate production account data from a supplied ADIF 3.1.8 document.
///
/// Use `--adif <file> --check` for parity or `--write` to update the committed data.
/// Existing native IDs are preserved; source descriptions are not copied. No
/// arguments or `--help` displays usage without writing files.
pub fn run(arguments: impl IntoIterator<Item = String>) -> anyhow::Result<()> {
    let mut source = None;
    let mut write = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" => {
                print!("{HELP}");
                return Ok(());
            }
            "--adif" => {
                source = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--adif requires a file"))?,
                ));
            }
            "--write" => write = true,
            "--check" => write = false,
            _ => anyhow::bail!("unsupported argument; {HELP}"),
        }
    }

    let Some(source) = source else {
        print!("{HELP}");
        return Ok(());
    };

    let html = std::fs::read_to_string(source)?;
    anyhow::ensure!(
        html.contains("ADIF Version 3.1.8"),
        "expected ADIF 3.1.8 reference document"
    );

    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../clacks-model/data/account-definitions.json");
    let previous = std::fs::read_to_string(&output)?;
    let existing: AccountDefinitions = serde_json::from_str(&previous)?;
    let next = build(contest_ids(&html)?, &existing)?;
    let encoded = format!("{}\n", serde_json::to_string_pretty(&next)?);

    if write {
        std::fs::write(output, encoded)?;
    } else {
        anyhow::ensure!(
            previous == encoded,
            "account definitions differ; regenerate with --write"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn extracts_ids_without_copying_descriptions() {
        let html = r#"<table id="Enumeration_Contest_ID"><tr><th>Contest-ID</th></tr><tr><td><a>TEST-CW</a></td><td>Not copied</td></tr><tr><td>HIST-CW&nbsp; (<a>import-only</a>)</td><td>History</td></tr></table>"#;
        assert_eq!(super::contest_ids(html).unwrap(), ["HIST-CW", "TEST-CW"]);
        assert!(super::contest_ids("not a contest table").is_err());
    }
}

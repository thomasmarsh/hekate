//! The benchmark matrix's two representations must agree.
//!
//! `docs/benchmark-matrix.md` is the readable matrix of record and
//! `docs/benchmark-matrix.json` is its machine-readable companion; the document
//! states that the JSON "carries the same modes, families, pairs, cells,
//! dispositions, and counts". Nothing checked that claim, so a cell could drift
//! between the two files, a fixture path could be checked in while its row stayed
//! in the planned table, or a path could be listed that does not exist — with
//! every other suite still green.
//!
//! This suite reads both representations from the repository and holds them to
//! each other:
//!
//! 1. the mode, family, and impossible-reason axes;
//! 2. every `(pair, family)` cell's disposition, class, and reason, and the
//!    disposition counts the document's audit section reports;
//! 3. every cell class's tolerance ids and fidelity presets, and the tolerance
//!    catalogue's id set;
//! 4. the fixture tables' ids and paths — every checked-in path exists on disk,
//!    every planned path does not, and every path §7.4 names resolves;
//! 5. that no Increment 2 path stays planned in either representation and that
//!    exactly the checked-in Increment 2 fixtures are named by both.
//!
//! It is a documentation-consistency gate. It runs no simulation, and it asserts
//! nothing about a `(pair, family)` cell's disposition beyond what the two
//! representations already state. It sits beside this suite's other
//! repository-artifact gates (`migration_regression.rs` enumerates
//! `scenarios/**/*.json5` the same way) because the CLI is what loads a matrix
//! fixture path and reports the artifacts the matrix bounds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::Value;

/// The Markdown matrix of record.
const MATRIX_MD: &str = include_str!("../../../docs/benchmark-matrix.md");
/// Its machine-readable companion.
const MATRIX_JSON: &str = include_str!("../../../docs/benchmark-matrix.json");

/// The §8 grid header's family label to the JSON's family key. The Markdown
/// spells two families with a slash or a hyphen; the JSON keys them with an
/// underscore, and both are the same family.
const FAMILY_KEYS: [(&str, &str); 8] = [
    ("following", "following"),
    ("crossing", "crossing"),
    ("merging", "merging"),
    ("overtaking", "overtaking"),
    ("head-on / opposing", "head-on_opposing"),
    ("shared-space", "shared_space"),
    ("stop-service", "stop_service"),
    ("unknown", "unknown"),
];

/// The repository root, from this crate's manifest directory.
fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The parsed machine-readable companion.
fn matrix_json() -> Value {
    serde_json::from_str(MATRIX_JSON).expect("the matrix JSON parses")
}

/// One numbered Markdown section or subsection, from its heading to the next
/// heading of either level.
///
/// `start` is the text after the heading's `## `/`### ` marker, for example
/// `8. Pairwise matrix`.
fn md_section(start: &str) -> &'static str {
    let h2 = format!("## {start}");
    let h3 = format!("### {start}");
    let mut begin = None;
    let mut end = MATRIX_MD.len();
    for (offset, _) in MATRIX_MD.match_indices('\n') {
        let line = MATRIX_MD[offset + 1..].lines().next().unwrap_or("");
        if line.starts_with(&h2) || line.starts_with(&h3) {
            begin = Some(offset + 1);
            break;
        }
    }
    let begin = begin.unwrap_or_else(|| panic!("the matrix must carry the `{start}` section"));
    for (offset, _) in MATRIX_MD[begin..].match_indices('\n') {
        let line = MATRIX_MD[begin + offset + 1..].lines().next().unwrap_or("");
        if line.starts_with("## ") || line.starts_with("### ") {
            end = begin + offset + 1;
            break;
        }
    }
    &MATRIX_MD[begin..end]
}

/// The rows of the first Markdown table in `section`, header first, separator
/// row dropped.
fn table_rows(section: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect();
        if cells
            .iter()
            .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        rows.push(cells);
    }
    rows
}

/// A table cell's value without its code-span backticks.
fn unquote(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_owned()
}

/// The strings a JSON array holds.
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("expected a JSON array, got {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("expected a JSON string, got {item}"))
                .to_owned()
        })
        .collect()
}

/// The `(key, value)` string pairs a JSON object holds.
fn string_pairs(value: &Value) -> BTreeMap<String, String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("expected a JSON object, got {value}"))
        .iter()
        .map(|(key, item)| {
            (
                key.clone(),
                item.as_str()
                    .unwrap_or_else(|| panic!("expected a JSON string at `{key}`, got {item}"))
                    .to_owned(),
            )
        })
        .collect()
}

/// The §8 grid's family label to the JSON's family key.
fn family_key(label: &str) -> &'static str {
    FAMILY_KEYS
        .iter()
        .find_map(|(markdown, key)| (*markdown == label).then_some(*key))
        .unwrap_or_else(|| panic!("`{label}` is not a matrix family"))
}

/// A §5 presets cell's abbreviation to the JSON's preset name.
fn preset_key(abbreviation: &str) -> &'static str {
    match abbreviation {
        "F" => "fast",
        "S" => "standard",
        "f" => "fine",
        other => panic!("`{other}` is not a fidelity preset"),
    }
}

/// The `(disposition, detail)` a §8 cell states: `S:<class>`, `I:<reason>`, or
/// `D:<increment>`.
fn split_cell(cell: &str) -> (&'static str, &str) {
    let (marker, detail) = cell.split_at(2);
    match marker {
        "S:" => ("supported", detail),
        "I:" => ("impossible", detail),
        "D:" => ("deferred", detail),
        other => panic!("`{other}` is not a disposition marker"),
    }
}

/// One `{a,b}` brace group expanded, the shorthand §7.1 uses for its two
/// pedestrian-priority experiment scenarios.
fn expand_braces(path: &str) -> Vec<String> {
    let Some(open) = path.find('{') else {
        return vec![path.to_owned()];
    };
    let close = path[open..]
        .find('}')
        .map(|offset| open + offset)
        .unwrap_or_else(|| panic!("the brace group in `{path}` must close"));
    let (head, tail) = (&path[..open], &path[close + 1..]);
    path[open + 1..close]
        .split(',')
        .map(|option| format!("{head}{}{tail}", option.trim()))
        .collect()
}

/// Every `scenarios/…json5` path a Markdown section names.
fn json5_paths(section: &str) -> BTreeSet<String> {
    section
        .split(|c: char| c.is_whitespace() || c == '`')
        .filter(|token| token.starts_with("scenarios/") && token.ends_with(".json5"))
        .map(str::to_owned)
        .collect()
}

/// The §4.1 table's `mode -> class` rows.
fn markdown_mode_classes() -> BTreeMap<String, String> {
    table_rows(md_section("4.1 Modes"))
        .into_iter()
        .skip(1)
        .map(|row| (unquote(&row[0]), unquote(&row[1])))
        .collect()
}

/// The §7.2 checked-in table's `id -> path` rows.
fn markdown_checked_in() -> BTreeMap<String, String> {
    table_rows(md_section("7.2 Checked-in fixtures"))
        .into_iter()
        .skip(1)
        .map(|row| (unquote(&row[0]), unquote(&row[1])))
        .collect()
}

/// The §7.3 planned table's paths.
fn markdown_planned_paths() -> BTreeSet<String> {
    table_rows(md_section("7.3 Planned fixtures"))
        .into_iter()
        .skip(1)
        .map(|row| unquote(&row[1]))
        .collect()
}

/// Every fixture path the JSON names, checked in or planned.
fn json_fixture_paths(json: &Value) -> BTreeSet<String> {
    let fixtures = &json["fixtures"];
    let mut paths: BTreeSet<String> = strings(&fixtures["existing"]).into_iter().collect();
    for group in [
        "checked_in_increment_1",
        "checked_in_increment_2",
        "checked_in_increment_3",
    ] {
        paths.extend(string_pairs(&fixtures[group]).into_values());
    }
    paths.extend(string_pairs(&fixtures["planned_patterns"]).into_values());
    paths
}

#[test]
fn the_representations_agree_on_the_mode_family_and_reason_axes() {
    let json = matrix_json();

    assert_eq!(
        string_pairs(&json["mode_classes"]),
        markdown_mode_classes(),
        "§4.1 and the JSON must state the same mode classes"
    );

    let families: Vec<String> = table_rows(md_section("4.2 Interaction families"))
        .into_iter()
        .skip(1)
        .map(|row| family_key(&row[0]).to_owned())
        .collect();
    assert_eq!(
        strings(&json["families"]),
        families,
        "§4.2 and the JSON must state the same families in the same order"
    );

    let reasons: BTreeSet<String> = table_rows(md_section("4.3 Impossible-reason codes"))
        .into_iter()
        .skip(1)
        .map(|row| unquote(&row[0]).trim_start_matches("I:").to_owned())
        .collect();
    assert_eq!(
        string_pairs(&json["impossible_reasons"])
            .into_keys()
            .collect::<BTreeSet<_>>(),
        reasons,
        "§4.3 and the JSON must define the same impossible-reason codes"
    );
}

#[test]
fn the_representations_agree_on_every_cell_disposition() {
    let json = matrix_json();
    let cells = json["cells"]
        .as_object()
        .expect("the JSON cells are an object");
    let rows = table_rows(md_section("8. Pairwise matrix"));
    let header = rows
        .first()
        .expect("§8 carries the grid table's header row");
    assert_eq!(header[0], "pair", "§8's first column is the pair");
    let families: Vec<&str> = header[1..].iter().map(|label| family_key(label)).collect();
    assert_eq!(
        families.iter().copied().collect::<BTreeSet<_>>().len(),
        families.len(),
        "every grid column is one distinct family"
    );

    let mut dispositions: BTreeMap<String, usize> = BTreeMap::new();
    let mut family_dispositions: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut pairs = BTreeSet::new();
    for row in rows.iter().skip(1) {
        let pair = &row[0];
        assert_eq!(row.len(), families.len() + 1, "`{pair}` fills every column");
        assert!(pairs.insert(pair.clone()), "`{pair}` appears once");
        let row_cells = cells
            .get(pair)
            .unwrap_or_else(|| panic!("the JSON must carry the `{pair}` row"));
        for (family, cell) in families.iter().zip(&row[1..]) {
            let (disposition, detail) = split_cell(cell);
            let entry = row_cells
                .get(family)
                .unwrap_or_else(|| panic!("the JSON must carry the `{pair}`/`{family}` cell"));
            assert_eq!(
                entry["disposition"], disposition,
                "`{pair}`/`{family}` states `{cell}`"
            );
            match disposition {
                "supported" => assert_eq!(
                    entry["class"], detail,
                    "`{pair}`/`{family}` states the class `{detail}`"
                ),
                "impossible" => assert_eq!(
                    entry["reason"], detail,
                    "`{pair}`/`{family}` states the reason `{detail}`"
                ),
                // No cell is deferred in this revision; the `D:` grammar is
                // retained, so its JSON form is only required to be present.
                _ => assert!(
                    entry.get("increment").is_some(),
                    "`{pair}`/`{family}` must carry the deferred increment"
                ),
            }
            *dispositions.entry(disposition.to_owned()).or_default() += 1;
            *family_dispositions
                .entry((family, disposition))
                .or_default() += 1;
        }
    }
    assert_eq!(
        pairs,
        cells.keys().cloned().collect::<BTreeSet<_>>(),
        "§8 and the JSON must carry the same pairs"
    );

    let reported_dispositions = json["disposition_counts"]
        .as_object()
        .expect("the JSON carries disposition counts");
    for (disposition, count) in &dispositions {
        assert_eq!(
            reported_dispositions
                .get(disposition)
                .and_then(Value::as_u64),
            Some(*count as u64),
            "the reported `{disposition}` count must equal the grid's"
        );
    }
    assert_eq!(
        reported_dispositions
            .values()
            .filter_map(Value::as_u64)
            .sum::<u64>(),
        dispositions
            .values()
            .map(|count| *count as u64)
            .sum::<u64>(),
        "every counted cell has a reported disposition"
    );

    let reported_family = json["family_disposition_counts"]
        .as_object()
        .expect("the JSON carries per-family disposition counts");
    for (family, disposition) in family_dispositions.keys() {
        let reported = reported_family
            .get(*family)
            .and_then(|counts| counts.get(*disposition))
            .and_then(Value::as_u64);
        assert_eq!(
            reported,
            Some(family_dispositions[&(*family, *disposition)] as u64),
            "the reported `{family}`/`{disposition}` count must equal the grid's"
        );
    }
}

#[test]
fn the_representations_agree_on_every_cell_class_and_tolerance() {
    let json = matrix_json();
    let classes = json["cell_classes"]
        .as_object()
        .expect("the JSON cell classes are an object");
    let tolerances = json["tolerances"]
        .as_object()
        .expect("the JSON tolerance catalogue is an object");

    let catalogue: BTreeSet<String> = table_rows(md_section("6. Tolerance catalogue"))
        .into_iter()
        .skip(1)
        .map(|row| unquote(&row[0]))
        .collect();
    assert_eq!(
        catalogue,
        tolerances.keys().cloned().collect::<BTreeSet<_>>(),
        "§6 and the JSON must catalogue the same tolerance ids"
    );

    let mut markdown_classes = BTreeSet::new();
    for section in [
        "5.1 Independent-mode classes",
        "5.2 Pairwise and mixed classes",
    ] {
        for row in table_rows(md_section(section)).into_iter().skip(1) {
            let class = unquote(&row[0]);
            let ids: Vec<String> = row[2].split(',').map(unquote).collect();
            let presets: Vec<String> = row[4]
                .split(',')
                .map(|p| preset_key(p.trim()).to_owned())
                .collect();
            let entry = classes
                .get(&class)
                .unwrap_or_else(|| panic!("the JSON must carry the `{class}` class"));
            assert_eq!(
                strings(&entry["tolerance"]),
                ids,
                "`{class}` must name the same tolerances in both representations"
            );
            assert_eq!(
                strings(&entry["presets"]),
                presets,
                "`{class}` must name the same presets in both representations"
            );
            for id in &ids {
                assert!(
                    catalogue.contains(id),
                    "`{class}` bounds `{id}`, which §6 must catalogue"
                );
            }
            assert!(markdown_classes.insert(class), "each class appears once");
        }
    }
    assert_eq!(
        markdown_classes,
        classes.keys().cloned().collect::<BTreeSet<_>>(),
        "§5 and the JSON must define the same cell classes"
    );

    // Every class a supported grid cell names is a defined class, and every
    // tolerance a class bounds is catalogued by both representations.
    for row in table_rows(md_section("8. Pairwise matrix"))
        .into_iter()
        .skip(1)
    {
        for cell in &row[1..] {
            let (disposition, detail) = split_cell(cell);
            if disposition == "supported" {
                assert!(
                    markdown_classes.contains(detail),
                    "the grid's `{detail}` class must be defined in §5"
                );
            }
        }
    }
}

#[test]
fn the_representations_agree_on_every_fixture_path() {
    let json = matrix_json();
    let fixtures = &json["fixtures"];

    let existing: BTreeSet<String> =
        table_rows(md_section("7.1 Existing Phase 1 fixtures (baseline)"))
            .into_iter()
            .skip(1)
            .flat_map(|row| expand_braces(&unquote(&row[1])))
            .collect();
    assert_eq!(
        existing,
        strings(&fixtures["existing"]).into_iter().collect(),
        "§7.1 and the JSON must name the same existing paths"
    );

    let mut checked_in = string_pairs(&fixtures["checked_in_increment_1"]);
    checked_in.extend(string_pairs(&fixtures["checked_in_increment_2"]));
    checked_in.extend(string_pairs(&fixtures["checked_in_increment_3"]));
    assert_eq!(
        checked_in,
        markdown_checked_in(),
        "§7.2 and the JSON must name the same checked-in ids and paths"
    );

    let planned = markdown_planned_paths();
    assert_eq!(
        planned,
        string_pairs(&fixtures["planned_patterns"])
            .into_values()
            .collect(),
        "§7.3 and the JSON must name the same planned paths"
    );

    for path in existing.iter().chain(checked_in.values()) {
        assert!(
            repo_path(path).is_file(),
            "a checked-in fixture path must exist: {path}"
        );
    }
    for path in &planned {
        assert!(
            !repo_path(path).exists(),
            "a planned fixture path must not be checked in yet: {path}"
        );
    }

    let named = json_fixture_paths(&json);
    for path in json5_paths(md_section(
        "7.4 Fixture resolution for a supported pairwise cell",
    )) {
        assert!(
            named.contains(&path),
            "§7.4 names a path the fixture tables do not: {path}"
        );
    }
}

#[test]
fn no_increment_2_evidence_remains_planned() {
    let json = matrix_json();
    let fixtures = &json["fixtures"];

    for path in string_pairs(&fixtures["planned_patterns"]).into_values() {
        assert!(
            !path.contains("phase2/inc2/"),
            "a checked-in Increment 2 path must not stay planned in the JSON: {path}"
        );
    }
    for path in markdown_planned_paths() {
        assert!(
            !path.contains("phase2/inc2/"),
            "a checked-in Increment 2 path must not stay planned in §7.3: {path}"
        );
    }

    let declared = string_pairs(&fixtures["checked_in_increment_2"]);
    assert_eq!(
        declared.keys().cloned().collect::<BTreeSet<_>>(),
        [
            "narrow_passing_v2",
            "motor_passing_narrow_v2",
            "motor_lane_change_v2",
            "narrow_wrong_way_v2"
        ]
        .map(str::to_owned)
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "the JSON's checked-in Increment 2 set is the wired fixture family"
    );
    for path in declared.values() {
        assert!(
            path.starts_with("scenarios/phase2/inc2/") && repo_path(path).is_file(),
            "a checked-in Increment 2 path must be the checked-in fixture: {path}"
        );
    }

    let markdown_increment_2: BTreeSet<String> = markdown_checked_in()
        .into_values()
        .filter(|path| path.contains("phase2/inc2/"))
        .collect();
    assert_eq!(
        markdown_increment_2,
        declared.values().cloned().collect(),
        "§7.2 and the JSON must name the same checked-in Increment 2 paths"
    );
}

// SPDX-License-Identifier: AGPL-3.0
//! Single-checkpoint relocation for defined zone ranges.
//!
//! A checkpoint stores compact fingerprints, not document or zone contents.
//! Unchanged literal hexword pairs may be relocated after surrounding edits;
//! manually changed pairs are always treated as authoritative.

use crate::{
    fast_ops::{fast_scan_content, ScannedSection},
    header::update_lines,
    zone_type::encode_hex_word,
    RegeditedError, Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CHECKPOINT_FORMAT: &str = "regedited-zone-checkpoint-v1";
const DIFF_FORMAT: &str = "regedited-zone-diff-v1";
const FINGERPRINT_LINES: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneCheckpoint {
    pub format: String,
    pub file: String,
    pub created_unix: u64,
    pub file_checksum: String,
    #[serde(default)]
    pub indexes: Vec<u64>,
    pub zones: Vec<CheckpointZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointZone {
    pub index: u64,
    pub slot: usize,
    pub start: u32,
    pub end: u32,
    pub start_hex: String,
    pub end_hex: String,
    pub zone_type: String,
    pub line_count: usize,
    pub content_checksum: String,
    pub leading: Vec<String>,
    pub trailing: Vec<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneDiff {
    pub format: String,
    pub file: String,
    pub created_unix: u64,
    pub checkpoint_path: String,
    pub base_file_checksum: String,
    pub current_file_checksum: String,
    pub updates: Vec<ZoneUpdate>,
    pub manual: Vec<ZoneIssue>,
    pub unresolved: Vec<ZoneIssue>,
    pub content_changed: Vec<ZoneIssue>,
    pub added_indexes: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneUpdate {
    pub index: u64,
    pub slot: usize,
    pub old_start: u32,
    pub old_end: u32,
    pub new_start: u32,
    pub new_end: u32,
    pub expected_start_hex: String,
    pub expected_end_hex: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneIssue {
    pub index: u64,
    pub slot: usize,
    pub reason: String,
}

pub fn checkpoint_path(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.rgd-state.json", file.display()))
}

pub fn diff_path(file: &Path) -> PathBuf {
    let canonical = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let key = checksum64_hex(canonical.to_string_lossy().as_bytes());
    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .take(40)
        .collect::<String>();
    std::env::temp_dir()
        .join("regedited")
        .join("zone-diffs")
        .join(format!("{}-{}.json", stem, key))
}

pub fn checkpoint_exists(file: &Path) -> bool {
    checkpoint_path(file).is_file()
}

pub fn save_checkpoint(file: &Path) -> Result<(ZoneCheckpoint, PathBuf)> {
    let checkpoint = build_checkpoint(file)?;
    let path = checkpoint_path(file);
    write_json(&path, &checkpoint)?;
    Ok((checkpoint, path))
}

pub fn save_checkpoint_preserving(
    file: &Path,
    unresolved: &[ZoneIssue],
) -> Result<(ZoneCheckpoint, PathBuf)> {
    let previous = load_checkpoint(file)?;
    let mut checkpoint = build_checkpoint(file)?;
    let preserved: BTreeSet<(u64, usize)> = unresolved
        .iter()
        .map(|issue| (issue.index, issue.slot))
        .collect();
    checkpoint
        .zones
        .retain(|zone| !preserved.contains(&(zone.index, zone.slot)));
    checkpoint.zones.extend(
        previous
            .zones
            .into_iter()
            .filter(|zone| preserved.contains(&(zone.index, zone.slot))),
    );
    checkpoint.zones.sort_by_key(|zone| (zone.index, zone.slot));
    let path = checkpoint_path(file);
    write_json(&path, &checkpoint)?;
    Ok((checkpoint, path))
}

pub fn load_checkpoint(file: &Path) -> Result<ZoneCheckpoint> {
    let path = checkpoint_path(file);
    let text = fs::read_to_string(&path).map_err(|error| {
        RegeditedError::Parse(format!(
            "No zone checkpoint for {}: {}. Run `rgd commit` first.",
            file.display(),
            error
        ))
    })?;
    let checkpoint: ZoneCheckpoint = serde_json::from_str(&text)
        .map_err(|error| RegeditedError::Parse(format!("Invalid checkpoint JSON: {}", error)))?;
    if checkpoint.format != CHECKPOINT_FORMAT {
        return Err(RegeditedError::Parse(format!(
            "Unsupported checkpoint format '{}'",
            checkpoint.format
        )));
    }
    Ok(checkpoint)
}

pub fn check(file: &Path) -> Result<(ZoneDiff, PathBuf)> {
    let checkpoint = load_checkpoint(file)?;
    let content = fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let line_hashes: Vec<String> = lines.iter().map(|line| hash_line(line)).collect();
    let scan = fast_scan_content(&content)?;
    let section_map = sections_by_index(&scan);
    let current_indexes: BTreeSet<u64> = scan.iter().map(|section| section.index).collect();
    let old_indexes: BTreeSet<u64> = if checkpoint.indexes.is_empty() {
        checkpoint.zones.iter().map(|zone| zone.index).collect()
    } else {
        checkpoint.indexes.iter().copied().collect()
    };

    let mut diff = ZoneDiff {
        format: DIFF_FORMAT.to_string(),
        file: normalized_file_name(file),
        created_unix: unix_time()?,
        checkpoint_path: checkpoint_path(file).display().to_string(),
        base_file_checksum: checkpoint.file_checksum.clone(),
        current_file_checksum: checksum64_hex(content.as_bytes()),
        updates: Vec::new(),
        manual: Vec::new(),
        unresolved: Vec::new(),
        content_changed: Vec::new(),
        added_indexes: current_indexes.difference(&old_indexes).copied().collect(),
    };

    for saved in &checkpoint.zones {
        let matches = section_map.get(&saved.index).cloned().unwrap_or_default();
        if matches.is_empty() {
            diff.manual.push(issue(saved, "index no longer exists"));
            continue;
        }
        if matches.len() != 1 {
            diff.unresolved
                .push(issue(saved, "registry index is ambiguous"));
            continue;
        }
        let section = matches[0];
        let Some(ascii_line) = lines.get(section.ascii_line) else {
            diff.unresolved
                .push(issue(saved, "hexword line is out of bounds"));
            continue;
        };
        let current_literals = match pair_literals(ascii_line, saved.slot) {
            Ok(value) => value,
            Err(error) => {
                diff.unresolved.push(issue(saved, &error.to_string()));
                continue;
            }
        };
        if current_literals.0 != saved.start_hex || current_literals.1 != saved.end_hex {
            diff.manual.push(issue(
                saved,
                "literal hexword pair changed since the checkpoint",
            ));
            continue;
        }

        let current_pair = section.zone_pairs[saved.slot - 1];
        let current_checksum = range_checksum(&lines, current_pair.0, current_pair.1);
        if current_checksum.as_deref() == Some(saved.content_checksum.as_str()) {
            continue;
        }

        match locate_zone(saved, &lines, &line_hashes) {
            Ok((new_start, new_end, method)) => {
                if new_start == current_pair.0 && new_end == current_pair.1 {
                    diff.content_changed.push(issue(
                        saved,
                        "zone content changed without moving its boundaries",
                    ));
                } else {
                    diff.updates.push(ZoneUpdate {
                        index: saved.index,
                        slot: saved.slot,
                        old_start: current_pair.0,
                        old_end: current_pair.1,
                        new_start,
                        new_end,
                        expected_start_hex: saved.start_hex.clone(),
                        expected_end_hex: saved.end_hex.clone(),
                        method,
                    });
                }
            }
            Err(reason) => diff.unresolved.push(issue(saved, &reason)),
        }
    }

    let path = diff_path(file);
    write_json(&path, &diff)?;
    Ok((diff, path))
}

pub fn load_diff(file: &Path) -> Result<ZoneDiff> {
    let path = diff_path(file);
    let text = fs::read_to_string(&path).map_err(|error| {
        RegeditedError::Parse(format!(
            "No pending zone diff for {}: {}. Run `rgd check` first.",
            file.display(),
            error
        ))
    })?;
    let diff: ZoneDiff = serde_json::from_str(&text)
        .map_err(|error| RegeditedError::Parse(format!("Invalid zone diff JSON: {}", error)))?;
    if diff.format != DIFF_FORMAT {
        return Err(RegeditedError::Parse(format!(
            "Unsupported zone diff format '{}'",
            diff.format
        )));
    }
    Ok(diff)
}

pub fn apply_diff(file: &Path, diff: &ZoneDiff) -> Result<String> {
    let content = fs::read_to_string(file)?;
    let checksum = checksum64_hex(content.as_bytes());
    if checksum != diff.current_file_checksum {
        return Err(RegeditedError::Parse(
            "The document changed after this diff was created. Run `rgd check` again.".to_string(),
        ));
    }

    let lines: Vec<&str> = content.lines().collect();
    let scan = fast_scan_content(&content)?;
    let section_map = sections_by_index(&scan);
    let mut changes: BTreeMap<usize, Vec<String>> = BTreeMap::new();

    for update in &diff.updates {
        let matches = section_map.get(&update.index).cloned().unwrap_or_default();
        if matches.len() != 1 {
            return Err(RegeditedError::Parse(format!(
                "Index {} is missing or ambiguous; run `rgd check` again",
                update.index
            )));
        }
        let section = matches[0];
        let raw_line = lines.get(section.ascii_line).ok_or_else(|| {
            RegeditedError::Parse(format!("Index {} hexword line is missing", update.index))
        })?;
        let current = pair_literals(raw_line, update.slot)?;
        if current.0 != update.expected_start_hex || current.1 != update.expected_end_hex {
            return Err(RegeditedError::Parse(format!(
                "Index {} zone {} was manually changed after check; refusing pull",
                update.index, update.slot
            )));
        }

        let parts = changes
            .entry(section.ascii_line)
            .or_insert_with(|| split_hexword_line(raw_line).unwrap_or_default());
        if parts.len() != 6 {
            return Err(RegeditedError::Parse(format!(
                "Index {} has an invalid hexword line",
                update.index
            )));
        }
        let zone_type = section.zone_types[update.slot - 1];
        let offset = (update.slot - 1) * 2;
        parts[offset] = encode_hex_word(update.new_start, zone_type);
        parts[offset + 1] = encode_hex_word(update.new_end, zone_type);
    }

    let line_changes: Vec<(usize, String)> = changes
        .into_iter()
        .map(|(line, parts)| (line, parts.join(" : ")))
        .collect();
    update_lines(&content, &line_changes)
}

pub fn clear_diff(file: &Path) -> Result<()> {
    let path = diff_path(file);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn build_checkpoint(file: &Path) -> Result<ZoneCheckpoint> {
    let content = fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let scan = fast_scan_content(&content)?;
    let indexes = scan.iter().map(|section| section.index).collect();
    let mut zones = Vec::new();

    for section in scan {
        let ascii_line = lines.get(section.ascii_line).ok_or_else(|| {
            RegeditedError::Parse(format!("Index {} hexword line is missing", section.index))
        })?;
        for slot_index in 0..3 {
            let (start, end) = section.zone_pairs[slot_index];
            if start == 0 && end == 0 {
                continue;
            }
            if start > end || end as usize >= lines.len() {
                return Err(RegeditedError::ZoneOutOfBounds {
                    line: end as usize,
                    max_lines: lines.len(),
                });
            }
            let slot = slot_index + 1;
            let (start_hex, end_hex) = pair_literals(ascii_line, slot)?;
            let start_usize = start as usize;
            let end_usize = end as usize;
            let zone_hashes = hash_slice(&lines[start_usize..=end_usize]);
            let leading = zone_hashes
                .iter()
                .take(FINGERPRINT_LINES)
                .cloned()
                .collect();
            let trailing_start = zone_hashes.len().saturating_sub(FINGERPRINT_LINES);
            let trailing = zone_hashes[trailing_start..].to_vec();
            let before_start = start_usize.saturating_sub(FINGERPRINT_LINES);
            let before = hash_slice(&lines[before_start..start_usize]);
            let after_start = end_usize + 1;
            let after_end = (after_start + FINGERPRINT_LINES).min(lines.len());
            let after = hash_slice(&lines[after_start..after_end]);

            zones.push(CheckpointZone {
                index: section.index,
                slot,
                start,
                end,
                start_hex,
                end_hex,
                zone_type: section.zone_types[slot_index].short().to_string(),
                line_count: end_usize - start_usize + 1,
                content_checksum: checksum_range(&lines, start_usize, end_usize),
                leading,
                trailing,
                before,
                after,
            });
        }
    }

    Ok(ZoneCheckpoint {
        format: CHECKPOINT_FORMAT.to_string(),
        file: normalized_file_name(file),
        created_unix: unix_time()?,
        file_checksum: checksum64_hex(content.as_bytes()),
        indexes,
        zones,
    })
}

fn locate_zone(
    saved: &CheckpointZone,
    lines: &[&str],
    hashes: &[String],
) -> std::result::Result<(u32, u32, String), String> {
    let leading_positions = sequence_positions(hashes, &saved.leading);
    let mut exact = Vec::new();
    for start in &leading_positions {
        let Some(end) = start.checked_add(saved.line_count.saturating_sub(1)) else {
            continue;
        };
        if end >= lines.len() || saved.trailing.len() > saved.line_count {
            continue;
        }
        let trailing_start = end + 1 - saved.trailing.len();
        if hashes[trailing_start..=end] == saved.trailing
            && checksum_range(lines, *start, end) == saved.content_checksum
        {
            exact.push((*start, end));
        }
    }
    dedup_pairs(&mut exact);
    if exact.len() == 1 {
        return checked_location(exact[0], "exact-content");
    }

    if !saved.before.is_empty() && !saved.after.is_empty() {
        let starts: Vec<usize> = sequence_positions(hashes, &saved.before)
            .into_iter()
            .map(|position| position + saved.before.len())
            .collect();
        let ends: Vec<usize> = sequence_positions(hashes, &saved.after)
            .into_iter()
            .filter_map(|position| position.checked_sub(1))
            .collect();
        let mut context_pairs = ordered_pairs(&starts, &ends);
        dedup_pairs(&mut context_pairs);
        if context_pairs.len() == 1 {
            return checked_location(context_pairs[0], "boundary-context");
        }
    }

    if !saved.leading.is_empty() && !saved.trailing.is_empty() {
        let starts = leading_positions;
        let ends: Vec<usize> = sequence_positions(hashes, &saved.trailing)
            .into_iter()
            .map(|position| position + saved.trailing.len() - 1)
            .collect();
        let mut boundary_pairs = ordered_pairs(&starts, &ends);
        dedup_pairs(&mut boundary_pairs);
        if boundary_pairs.len() == 1 {
            return checked_location(boundary_pairs[0], "zone-boundaries");
        }
    }

    if exact.len() > 1 {
        Err("zone content has multiple exact locations".to_string())
    } else {
        Err("zone boundaries could not be located without guessing".to_string())
    }
}

fn checked_location(
    pair: (usize, usize),
    method: &str,
) -> std::result::Result<(u32, u32, String), String> {
    if pair.0 > pair.1 || pair.1 > 0x0FFF_FFFF {
        return Err("located zone range is invalid".to_string());
    }
    Ok((pair.0 as u32, pair.1 as u32, method.to_string()))
}

fn ordered_pairs(starts: &[usize], ends: &[usize]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for start in starts {
        for end in ends {
            if start <= end {
                pairs.push((*start, *end));
            }
        }
    }
    pairs
}

fn dedup_pairs(pairs: &mut Vec<(usize, usize)>) {
    pairs.sort_unstable();
    pairs.dedup();
}

fn sequence_positions(haystack: &[String], needle: &[String]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

fn sections_by_index(scan: &[ScannedSection]) -> BTreeMap<u64, Vec<&ScannedSection>> {
    let mut result: BTreeMap<u64, Vec<&ScannedSection>> = BTreeMap::new();
    for section in scan {
        result.entry(section.index).or_default().push(section);
    }
    result
}

fn pair_literals(line: &str, slot: usize) -> Result<(String, String)> {
    if !(1..=3).contains(&slot) {
        return Err(RegeditedError::Parse(format!(
            "Zone slot {} out of range; use 1-3",
            slot
        )));
    }
    let parts = split_hexword_line(line)?;
    let offset = (slot - 1) * 2;
    Ok((parts[offset].clone(), parts[offset + 1].clone()))
}

fn split_hexword_line(line: &str) -> Result<Vec<String>> {
    let parts: Vec<String> = line
        .split(':')
        .map(|part| part.trim().to_string())
        .collect();
    if parts.len() != 6 {
        return Err(RegeditedError::Parse(
            "Hexword line must contain exactly six values".to_string(),
        ));
    }
    Ok(parts)
}

fn issue(zone: &CheckpointZone, reason: &str) -> ZoneIssue {
    ZoneIssue {
        index: zone.index,
        slot: zone.slot,
        reason: reason.to_string(),
    }
}

fn range_checksum(lines: &[&str], start: u32, end: u32) -> Option<String> {
    let start = start as usize;
    let end = end as usize;
    if start > end || end >= lines.len() {
        return None;
    }
    Some(checksum_range(lines, start, end))
}

fn checksum_range(lines: &[&str], start: usize, end: usize) -> String {
    let mut hasher = fxhash::FxHasher64::default();
    for (offset, line) in lines[start..=end].iter().enumerate() {
        if offset > 0 {
            hasher.write(b"\n");
        }
        hasher.write(line.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

fn hash_slice(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| hash_line(line)).collect()
}

fn hash_line(line: &str) -> String {
    checksum64_hex(line.as_bytes())
}

fn checksum64_hex(data: &[u8]) -> String {
    let mut hasher = fxhash::FxHasher64::default();
    hasher.write(data);
    format!("{:016x}", hasher.finish())
}

fn normalized_file_name(file: &Path) -> String {
    file.canonicalize()
        .unwrap_or_else(|_| file.to_path_buf())
        .display()
        .to_string()
}

fn unix_time() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| RegeditedError::Parse(format!("System clock error: {}", error)))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| RegeditedError::Parse(format!("JSON serialization failed: {}", error)))?;
    fs::write(path, text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn base_document() -> String {
        [
            "# document",
            "regedited open",
            "index: 1",
            "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
            "summary one",
            "summary two",
            "summary three",
            "---",
            "alpha",
            "beta",
            "gamma",
            "tail",
        ]
        .join("\n")
    }

    fn inserted_index_document(pair: &str) -> String {
        [
            "# document",
            "regedited open",
            "index: 2",
            "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            "0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0",
            "new one",
            "new two",
            "new three",
            "---",
            "new body",
            "regedited open",
            "index: 1",
            pair,
            "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
            "summary one",
            "summary two",
            "summary three",
            "---",
            "alpha",
            "beta",
            "gamma",
            "tail",
        ]
        .join("\n")
    }

    #[test]
    fn checkpoint_tracks_only_nonzero_zones() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        let (checkpoint, path) = save_checkpoint(&file).unwrap();
        assert!(path.is_file());
        assert_eq!(checkpoint.zones.len(), 1);
        assert_eq!(checkpoint.indexes, vec![1]);
        assert_eq!(checkpoint.zones[0].slot, 1);
        assert_eq!(checkpoint.zones[0].line_count, 3);
        assert!(checkpoint.zones[0].leading.len() <= FINGERPRINT_LINES);
    }

    #[test]
    fn zero_zone_index_is_not_reported_as_new() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        let initial = inserted_index_document(
            "0x0000012 : 0x0000014 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
        );
        fs::write(&file, &initial).unwrap();
        save_checkpoint(&file).unwrap();

        let (diff, _) = check(&file).unwrap();
        assert!(diff.added_indexes.is_empty());
    }

    #[test]
    fn inserted_index_relocates_unchanged_zone() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        fs::write(
            &file,
            inserted_index_document(
                "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            ),
        )
        .unwrap();

        let (diff, _) = check(&file).unwrap();
        assert_eq!(diff.updates.len(), 1);
        assert_eq!(
            (diff.updates[0].new_start, diff.updates[0].new_end),
            (18, 20)
        );
        assert_eq!(diff.updates[0].method, "exact-content");
    }

    #[test]
    fn manual_hexword_change_is_never_overwritten() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        fs::write(
            &file,
            inserted_index_document(
                "0x0000012 : 0x0000014 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            ),
        )
        .unwrap();

        let (diff, _) = check(&file).unwrap();
        assert!(diff.updates.is_empty());
        assert_eq!(diff.manual.len(), 1);
    }

    #[test]
    fn changed_zone_content_uses_unchanged_boundaries() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        let edited = base_document().replace("alpha\nbeta\ngamma", "alpha\ninserted\nbeta\ngamma");
        fs::write(&file, edited).unwrap();

        let (diff, _) = check(&file).unwrap();
        assert_eq!(diff.updates.len(), 1);
        assert_eq!(
            (diff.updates[0].new_start, diff.updates[0].new_end),
            (9, 12)
        );
        assert_eq!(diff.updates[0].method, "boundary-context");
    }

    #[test]
    fn pull_rewrites_only_the_unchanged_literal_pair() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        fs::write(
            &file,
            inserted_index_document(
                "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            ),
        )
        .unwrap();
        let (diff, _) = check(&file).unwrap();
        let updated = apply_diff(&file, &diff).unwrap();
        assert!(updated
            .contains("0x0000012 : 0x0000014 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000"));
    }

    #[test]
    fn stale_diff_is_rejected() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        fs::write(
            &file,
            inserted_index_document(
                "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            ),
        )
        .unwrap();
        let (diff, _) = check(&file).unwrap();
        fs::write(
            &file,
            format!("{}\nlate edit", fs::read_to_string(&file).unwrap()),
        )
        .unwrap();
        assert!(apply_diff(&file, &diff).is_err());
    }

    #[test]
    fn ambiguous_relocation_is_never_guessed() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        let duplicate = [
            "prefix",
            &base_document(),
            "summary one",
            "summary two",
            "summary three",
            "---",
            "alpha",
            "beta",
            "gamma",
            "tail",
        ]
        .join("\n");
        fs::write(&file, duplicate).unwrap();

        let (diff, _) = check(&file).unwrap();
        assert!(diff.updates.is_empty());
        assert_eq!(diff.unresolved.len(), 1);
        assert!(diff.unresolved[0].reason.contains("multiple exact"));
    }

    #[test]
    fn unresolved_fingerprint_can_survive_checkpoint_advancement() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("doc.md");
        fs::write(&file, base_document()).unwrap();
        save_checkpoint(&file).unwrap();
        fs::write(
            &file,
            inserted_index_document(
                "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            ),
        )
        .unwrap();
        let issue = ZoneIssue {
            index: 1,
            slot: 1,
            reason: "test preservation".to_string(),
        };

        save_checkpoint_preserving(&file, &[issue]).unwrap();
        let (diff, _) = check(&file).unwrap();
        assert_eq!(diff.updates.len(), 1);
        assert_eq!(diff.updates[0].new_start, 18);
    }

    #[test]
    #[ignore = "one-million-line relocation stress test"]
    fn million_line_relocation_keeps_checkpoint_compact() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("large.md");
        let target_start = 900_000usize;
        let mut lines = vec![
            "regedited open".to_string(),
            "index: 700".to_string(),
            "0x00DBBA0 : 0x00DBBA2 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000".to_string(),
            "0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0".to_string(),
            "large".to_string(),
            "relocation".to_string(),
            "fixture".to_string(),
            "---".to_string(),
        ];
        while lines.len() < target_start {
            lines.push(format!("line {}", lines.len()));
        }
        lines.extend(["target alpha", "target beta", "target gamma"].map(String::from));
        while lines.len() < 1_000_000 {
            lines.push(format!("line {}", lines.len()));
        }
        fs::write(&file, lines.join("\n")).unwrap();
        let (_, checkpoint) = save_checkpoint(&file).unwrap();
        assert!(fs::metadata(checkpoint).unwrap().len() < 4096);

        lines.splice(0..0, (0..10_000).map(|line| format!("inserted {line}")));
        fs::write(&file, lines.join("\n")).unwrap();
        let (diff, _) = check(&file).unwrap();
        assert_eq!(diff.updates.len(), 1);
        assert_eq!(diff.updates[0].new_start, 910_000);
        assert_eq!(diff.updates[0].new_end, 910_002);
    }
}

// SPDX-License-Identifier: AGPL-3.0
//! # Transaction System
//!
//! Batch multiple operations into a single atomic unit. All changes are
//! staged in memory, logged to WAL, and applied together on commit.
//! If any step fails, the entire transaction rolls back.
//!
//! ## Design
//!
//! ```text
//! regedited tx begin config.regd     → creates transaction + WAL
//! regedited set-num config.regd ...  → staged (WAL logged, NOT applied)
//! regedited set-str config.regd ...  → staged
//! regedited tx commit config.regd    → applies all, commits WAL
//! # or
//! regedited tx rollback config.regd  → discards all, removes WAL
//! ```
//!
//! ## Why This Matters
//!
//! The Windows Registry cannot do transactional batch edits. If a Group Policy
//! update sets 50 keys and fails on #47, the system is left in an inconsistent
//! state. Regedited transactions guarantee all-or-nothing semantics.

use crate::{
    wal::{Wal, WalOperation},
    RegeditedError, Result,
};
use std::path::{Path, PathBuf};

/// Transaction state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction has been started but no operations staged
    Started,
    /// Operations have been staged
    Staging,
    /// Transaction committed successfully
    Committed,
    /// Transaction rolled back
    RolledBack,
    /// Transaction failed (error occurred)
    Failed(String),
}

/// A Regedited transaction
///
/// Buffers operations in memory, logs them to WAL, and applies them
/// atomically on commit. On failure, all changes are rolled back.
pub struct Transaction {
    /// Document file path
    doc_path: PathBuf,
    /// WAL for durability
    wal: Wal,
    /// Staged operations
    staged: Vec<StagedOperation>,
    /// Current state
    state: TransactionState,
    /// Human-readable description
    description: String,
}

/// A staged operation with its context
#[derive(Debug, Clone)]
pub struct StagedOperation {
    /// Sequence number within the transaction
    pub seq: usize,
    /// The WAL operation
    pub op: WalOperation,
    /// Human-readable description
    pub description: String,
}

impl Transaction {
    /// Begin a new transaction for a document
    pub fn begin<P: AsRef<Path>>(doc_path: P) -> Result<Self> {
        let doc_path = doc_path.as_ref().to_path_buf();

        // Check if a transaction already exists
        if Wal::exists_for(&doc_path) && !Wal::is_committed(&doc_path)? {
            return Err(RegeditedError::Parse(format!(
                "Transaction already in progress for {}. Run 'tx rollback' or 'tx commit' first.",
                doc_path.display()
            )));
        }

        let wal = Wal::open(&doc_path)?;

        Ok(Self {
            doc_path,
            wal,
            staged: Vec::new(),
            state: TransactionState::Started,
            description: "Regedited transaction".to_string(),
        })
    }

    /// Stage a numeric value change
    pub fn stage_set_num(
        &mut self,
        section: &str,
        index: usize,
        old_value: i64,
        new_value: i64,
    ) -> Result<()> {
        let op = WalOperation::SetNum {
            section: section.to_string(),
            index,
            old_value,
            new_value,
        };
        self.stage(op)
    }

    /// Stage a string value change
    pub fn stage_set_str(
        &mut self,
        section: &str,
        index: usize,
        old_value: &str,
        new_value: &str,
    ) -> Result<()> {
        let op = WalOperation::SetStr {
            section: section.to_string(),
            index,
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
        };
        self.stage(op)
    }

    /// Stage a zone change
    pub fn stage_set_zone(
        &mut self,
        section: &str,
        zone: usize,
        old_range: (u32, u32, String),
        new_range: (u32, u32, String),
    ) -> Result<()> {
        let op = WalOperation::SetZone {
            section: section.to_string(),
            zone,
            old_range,
            new_range,
        };
        self.stage(op)
    }

    /// Stage a section add
    pub fn stage_section_add(&mut self, section: &str) -> Result<()> {
        let op = WalOperation::SectionAdd {
            section: section.to_string(),
        };
        self.stage(op)
    }

    /// Stage a section remove
    pub fn stage_section_remove(&mut self, section: &str, old_content: &str) -> Result<()> {
        let op = WalOperation::SectionRemove {
            section: section.to_string(),
            old_content: old_content.to_string(),
        };
        self.stage(op)
    }

    /// Stage a zone content replacement
    pub fn stage_zone_replace(
        &mut self,
        section: &str,
        zone: usize,
        old_content: &str,
        new_content: &str,
    ) -> Result<()> {
        let op = WalOperation::ZoneReplace {
            section: section.to_string(),
            zone,
            old_content: old_content.to_string(),
            new_content: new_content.to_string(),
        };
        self.stage(op)
    }

    /// Stage a generic WAL operation
    fn stage(&mut self, op: WalOperation) -> Result<()> {
        if matches!(
            self.state,
            TransactionState::Committed | TransactionState::RolledBack
        ) {
            return Err(RegeditedError::Parse(
                "Cannot stage to a completed transaction".to_string(),
            ));
        }

        let seq = self.staged.len() + 1;
        let description = op.description();

        // Log to WAL immediately (durability)
        self.wal.append(op.clone())?;

        self.staged.push(StagedOperation {
            seq,
            op,
            description,
        });
        self.state = TransactionState::Staging;

        Ok(())
    }

    /// Commit all staged operations
    ///
    /// This applies the WAL (marks it committed) and transitions the state.
    /// The actual file changes must be applied separately using the staged ops.
    pub fn commit(&mut self) -> Result<Vec<WalOperation>> {
        if self.staged.is_empty() {
            self.state = TransactionState::Committed;
            self.wal.cleanup()?;
            return Ok(Vec::new());
        }

        if !matches!(
            self.state,
            TransactionState::Started | TransactionState::Staging
        ) {
            return Err(RegeditedError::Parse(format!(
                "Cannot commit transaction in state: {:?}",
                self.state
            )));
        }

        // Commit the WAL (durability marker)
        self.wal.commit()?;
        self.state = TransactionState::Committed;

        // Return the operations for application
        let ops: Vec<WalOperation> = self.staged.iter().map(|s| s.op.clone()).collect();
        Ok(ops)
    }

    /// Rollback all staged operations
    pub fn rollback(&mut self) -> Result<()> {
        self.wal.rollback()?;
        self.staged.clear();
        self.state = TransactionState::RolledBack;
        Ok(())
    }

    /// Get staged operations
    pub fn staged(&self) -> &[StagedOperation] {
        &self.staged
    }

    /// Get the number of staged operations
    pub fn len(&self) -> usize {
        self.staged.len()
    }

    /// Check if any operations are staged
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty()
    }

    /// Get current state
    pub fn state(&self) -> &TransactionState {
        &self.state
    }

    /// Get document path
    pub fn doc_path(&self) -> &Path {
        &self.doc_path
    }

    /// Format transaction summary for display
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("Transaction for: {}", self.doc_path.display()),
            format!("Description: {}", self.description),
            format!("State: {:?}", self.state),
            format!("Staged operations: {}", self.staged.len()),
            String::new(),
        ];
        for op in &self.staged {
            lines.push(format!("  {:2}. {}", op.seq, op.description));
        }
        lines.join("\n")
    }

    /// Check if a transaction is in progress for a document
    pub fn is_in_progress<P: AsRef<Path>>(doc_path: P) -> Result<bool> {
        let doc_path = doc_path.as_ref();
        if !Wal::exists_for(doc_path) {
            return Ok(false);
        }
        // WAL exists but not committed = transaction in progress
        Ok(!Wal::is_committed(doc_path)?)
    }
}

/// Transaction manager for handling multiple concurrent transactions
pub struct TransactionManager {
    /// Active transactions keyed by document path
    active: std::collections::BTreeMap<String, Transaction>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new() -> Self {
        Self {
            active: std::collections::BTreeMap::new(),
        }
    }

    /// Begin a transaction for a document
    pub fn begin<P: AsRef<Path>>(&mut self, doc_path: P) -> Result<&Transaction> {
        let path = doc_path.as_ref().to_string_lossy().to_string();
        if self.active.contains_key(&path) {
            return Err(RegeditedError::Parse(format!(
                "Transaction already active for {}",
                path
            )));
        }
        let tx = Transaction::begin(doc_path)?;
        self.active.insert(path.clone(), tx);
        Ok(self.active.get(&path).unwrap())
    }

    /// Get an active transaction
    pub fn get<P: AsRef<Path>>(&self, doc_path: P) -> Option<&Transaction> {
        let path = doc_path.as_ref().to_string_lossy().to_string();
        self.active.get(&path)
    }

    /// Get a mutable transaction
    pub fn get_mut<P: AsRef<Path>>(&mut self, doc_path: P) -> Option<&mut Transaction> {
        let path = doc_path.as_ref().to_string_lossy().to_string();
        self.active.get_mut(&path)
    }

    /// Commit and remove a transaction
    pub fn commit<P: AsRef<Path>>(&mut self, doc_path: P) -> Result<Vec<WalOperation>> {
        let path = doc_path.as_ref().to_string_lossy().to_string();
        if let Some(tx) = self.active.get_mut(&path) {
            let ops = tx.commit()?;
            self.active.remove(&path);
            Ok(ops)
        } else {
            Err(RegeditedError::Parse(format!(
                "No active transaction for {}",
                path
            )))
        }
    }

    /// Rollback and remove a transaction
    pub fn rollback<P: AsRef<Path>>(&mut self, doc_path: P) -> Result<()> {
        let path = doc_path.as_ref().to_string_lossy().to_string();
        if let Some(tx) = self.active.get_mut(&path) {
            tx.rollback()?;
            self.active.remove(&path);
            Ok(())
        } else {
            Err(RegeditedError::Parse(format!(
                "No active transaction for {}",
                path
            )))
        }
    }

    /// List all active transactions
    pub fn active_docs(&self) -> Vec<&str> {
        self.active.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_begin_commit() {
        let tmp = std::env::temp_dir().join("regedited_test_tx.md");
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        let mut tx = Transaction::begin(&tmp).unwrap();
        assert!(tx.is_empty());
        assert!(matches!(tx.state(), TransactionState::Started));

        tx.stage_set_num("Config", 0, 10, 20).unwrap();
        tx.stage_set_num("Config", 1, 30, 40).unwrap();

        assert_eq!(tx.len(), 2);
        assert!(matches!(tx.state(), TransactionState::Staging));

        let ops = tx.commit().unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(tx.state(), TransactionState::Committed));

        // Cleanup
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&wal_path);
    }

    #[test]
    fn test_transaction_rollback() {
        let tmp = std::env::temp_dir().join("regedited_test_tx_rollback.md");
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        let mut tx = Transaction::begin(&tmp).unwrap();
        tx.stage_set_str("Config", 0, "old", "new").unwrap();
        assert_eq!(tx.len(), 1);

        tx.rollback().unwrap();
        assert!(tx.is_empty());
        assert!(matches!(tx.state(), TransactionState::RolledBack));

        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&wal_path);
    }

    #[test]
    fn test_transaction_manager() {
        let tmp = std::env::temp_dir().join("regedited_test_tx_mgr.md");
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        let mut mgr = TransactionManager::new();
        mgr.begin(&tmp).unwrap();

        {
            let tx = mgr.get_mut(&tmp).unwrap();
            tx.stage_set_num("Config", 0, 1, 2).unwrap();
        }

        let ops = mgr.commit(&tmp).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(mgr.get(&tmp).is_none());

        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&wal_path);
    }

    #[test]
    fn test_transaction_duplicate_prevention() {
        let tmp = std::env::temp_dir().join("regedited_test_tx_dup.md");
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        let mut mgr = TransactionManager::new();
        mgr.begin(&tmp).unwrap();

        // Should fail — transaction already active
        assert!(mgr.begin(&tmp).is_err());

        // Rollback and try again
        mgr.rollback(&tmp).unwrap();
        assert!(mgr.begin(&tmp).is_ok());

        mgr.rollback(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&wal_path);
    }
}

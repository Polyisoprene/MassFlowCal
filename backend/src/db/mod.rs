use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use crate::error::EngineError;
use crate::models::component::ComponentDef;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, EngineError> {
        let dir = Path::new(path).parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| EngineError::Database(e.to_string()))?;

        let conn = Connection::open(path)
            .map_err(|e| EngineError::Database(e.to_string()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| EngineError::Database(e.to_string()))?;

        let db = Self { conn: Mutex::new(conn) };
        db.init_tables()?;
        db.seed_defaults()?;
        Ok(db)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, EngineError> {
        self.conn.lock().map_err(|e| EngineError::Database(e.to_string()))
    }

    fn init_tables(&self) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS components (
                cas_number    TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                formula       TEXT NOT NULL DEFAULT '',
                mol_weight    REAL NOT NULL,
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS flows (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                graph_json    TEXT NOT NULL,
                created_at    TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        ).map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<(), EngineError> {
        let c = self.lock()?;
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM components", [], |r| r.get(0))
            .unwrap_or(0);

        if count > 0 {
            return Ok(());
        }

        c.execute_batch(include_str!("../../data/seed.sql"))
            .map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    // ==================== 组分 CRUD ====================

    pub fn get_all_components(&self) -> Result<Vec<ComponentDef>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT cas_number, name, formula, mol_weight FROM components ORDER BY name")
            .map_err(|e| EngineError::Database(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            Ok(ComponentDef {
                cas_number: row.get(0)?,
                name: row.get(1)?,
                formula: row.get(2)?,
                molecular_weight: row.get(3)?,
            })
        }).map_err(|e| EngineError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| EngineError::Database(e.to_string()))
    }

    pub fn get_component(&self, cas: &str) -> Result<Option<ComponentDef>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT cas_number, name, formula, mol_weight FROM components WHERE cas_number = ?1")
            .map_err(|e| EngineError::Database(e.to_string()))?;

        let mut rows = stmt.query_map([cas], |row| {
            Ok(ComponentDef {
                cas_number: row.get(0)?,
                name: row.get(1)?,
                formula: row.get(2)?,
                molecular_weight: row.get(3)?,
            })
        }).map_err(|e| EngineError::Database(e.to_string()))?;

        rows.next().transpose()
            .map_err(|e| EngineError::Database(e.to_string()))
    }

    pub fn get_mw(&self, cas: &str) -> Result<f64, EngineError> {
        let c = self.lock()?;
        c.query_row(
            "SELECT mol_weight FROM components WHERE cas_number = ?1",
            [cas],
            |row| row.get(0),
        ).map_err(|_| EngineError::UnknownComponent(cas.to_string()))
    }

    pub fn insert_component(&self, comp: &ComponentDef) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute(
            "INSERT OR REPLACE INTO components (cas_number, name, formula, mol_weight) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![comp.cas_number, comp.name, comp.formula, comp.molecular_weight],
        ).map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn update_component(&self, comp: &ComponentDef) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute(
            "UPDATE components SET name=?2, formula=?3, mol_weight=?4 WHERE cas_number=?1",
            rusqlite::params![comp.cas_number, comp.name, comp.formula, comp.molecular_weight],
        ).map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_component(&self, cas: &str) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute("DELETE FROM components WHERE cas_number = ?1", [cas])
            .map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    // ==================== 流程 CRUD ====================

    pub fn get_all_flows(&self) -> Result<Vec<FlowSummary>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT id, name, updated_at FROM flows ORDER BY updated_at DESC")
            .map_err(|e| EngineError::Database(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            Ok(FlowSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                updated_at: row.get(2)?,
            })
        }).map_err(|e| EngineError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| EngineError::Database(e.to_string()))
    }

    pub fn get_flow(&self, id: &str) -> Result<Option<String>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT graph_json FROM flows WHERE id = ?1")
            .map_err(|e| EngineError::Database(e.to_string()))?;

        let mut rows = stmt.query_map([id], |row| row.get(0))
            .map_err(|e| EngineError::Database(e.to_string()))?;

        rows.next().transpose()
            .map_err(|e| EngineError::Database(e.to_string()))
    }

    pub fn save_flow(&self, id: &str, name: &str, json: &str) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute(
            "INSERT INTO flows (id, name, graph_json, updated_at) VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET name=?2, graph_json=?3, updated_at=datetime('now')",
            rusqlite::params![id, name, json],
        ).map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_flow(&self, id: &str) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute("DELETE FROM flows WHERE id = ?1", [id])
            .map_err(|e| EngineError::Database(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowSummary {
    pub id: String,
    pub name: String,
    pub updated_at: String,
}

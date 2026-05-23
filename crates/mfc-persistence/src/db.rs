use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use mfc_core::component::ComponentDef;
use mfc_core::error::EngineError;
use mfc_core::traits::ComponentProvider;

pub struct SqliteDatabase {
    conn: Mutex<Connection>,
}

fn db_err(e: rusqlite::Error) -> EngineError {
    EngineError::Database(e.to_string())
}

impl SqliteDatabase {
    pub fn open(path: &str) -> Result<Self, EngineError> {
        let dir = Path::new(path).parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| EngineError::Database(e.to_string()))?;

        let conn = Connection::open(path)
            .map_err(db_err)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(db_err)?;

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
            );"
        ).map_err(db_err)?;
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

        c.execute_batch(include_str!("seed.sql"))
            .map_err(db_err)?;
        Ok(())
    }

    // ==================== 组分 CRUD ====================

    pub fn insert_component(&self, comp: &ComponentDef) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute(
            "INSERT OR REPLACE INTO components (cas_number, name, formula, mol_weight) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![comp.cas_number, comp.name, comp.formula, comp.molecular_weight],
        ).map_err(db_err)?;
        Ok(())
    }

    pub fn update_component(&self, comp: &ComponentDef) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute(
            "UPDATE components SET name=?2, formula=?3, mol_weight=?4 WHERE cas_number=?1",
            rusqlite::params![comp.cas_number, comp.name, comp.formula, comp.molecular_weight],
        ).map_err(db_err)?;
        Ok(())
    }

    pub fn delete_component(&self, cas: &str) -> Result<(), EngineError> {
        let c = self.lock()?;
        c.execute("DELETE FROM components WHERE cas_number = ?1", [cas])
            .map_err(db_err)?;
        Ok(())
    }

}

impl ComponentProvider for SqliteDatabase {
    fn get_mw(&self, cas: &str) -> Result<f64, EngineError> {
        let c = self.lock()?;
        c.query_row(
            "SELECT mol_weight FROM components WHERE cas_number = ?1",
            [cas],
            |row| row.get(0),
        ).map_err(|_| EngineError::UnknownComponent(cas.to_string()))
    }

    fn get_component(&self, cas: &str) -> Result<Option<ComponentDef>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT cas_number, name, formula, mol_weight FROM components WHERE cas_number = ?1")
            .map_err(db_err)?;

        let mut rows = stmt.query_map([cas], |row| {
            Ok(ComponentDef {
                cas_number: row.get(0)?,
                name: row.get(1)?,
                formula: row.get(2)?,
                molecular_weight: row.get(3)?,
            })
        }).map_err(db_err)?;

        rows.next().transpose()
            .map_err(db_err)
    }

    fn get_all_components(&self) -> Result<Vec<ComponentDef>, EngineError> {
        let c = self.lock()?;
        let mut stmt = c
            .prepare("SELECT cas_number, name, formula, mol_weight FROM components ORDER BY name")
            .map_err(db_err)?;

        let rows = stmt.query_map([], |row| {
            Ok(ComponentDef {
                cas_number: row.get(0)?,
                name: row.get(1)?,
                formula: row.get(2)?,
                molecular_weight: row.get(3)?,
            })
        }).map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_err)
    }
}


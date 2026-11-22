//! Shared cleanup for isolated model API fixtures; never removes another stream.

/// Release only this test's generated tables, registry and source stream, even after a panic.
pub(super) struct Cleanup {
    pub(super) namespace: String,
    pub(super) stream: String,
    pub(super) models: Vec<cantongrid_models::Model>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let Ok(mut client) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL") else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            for model in &self.models {
                tx.batch_execute(&format!(
                    "DROP TABLE IF EXISTS cantongrid_models.\"{}\"",
                    model.sql_plan()?.table
                ))?;
            }
            for table in [
                "model_rebuild_dirty",
                "model_rebuilds",
                "model_bindings",
                "outbox",
                "events",
                "contracts",
                "batches",
                "streams",
            ] {
                tx.execute(
                    &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                    &[&self.stream],
                )?;
            }
            tx.execute(
                "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
                &[&self.namespace],
            )?;
            tx.execute(
                "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                &[&self.namespace],
            )?;
            for model in &self.models {
                tx.execute(
                    "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1",
                    &[&model.fingerprint()?],
                )?;
            }
            Ok(())
        })();
        if result.is_ok() {
            let _ = tx.commit();
        }
    }
}

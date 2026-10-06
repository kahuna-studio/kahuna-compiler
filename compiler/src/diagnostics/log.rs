use salsa::Accumulator;

#[salsa::accumulator]
pub struct Log(pub String);

impl Log {
    pub fn info(db: &dyn salsa::Database, message: String) {
        Log(message).accumulate(db);
    }
}
